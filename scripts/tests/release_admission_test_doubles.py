"""Register and inspect the cmd-mox doubles one gate run reaches.

This module owns the boundary the release-admission runtime tests configure:
the two documented command adapters, ``gh`` and ``git``, the clock program, and
the small wrapper that turns a cmd-mox double into something a test can state a
statement about. :mod:`release_admission_test_harness` runs the gate; this
module decides what the gate runs and reads back what it ran.

Four resolution rules are deliberate.

*Standard input is always closed.* A cmd-mox shim reads its standard input
whenever that is not a terminal and forwards it over IPC. A child that merely
inherits the test run's standard input therefore deadlocks on the first shim it
runs, which is how this harness first failed. Every invocation passes
``DEVNULL``, and a test that cares about standard input asserts on the shim's
own record of what it received.

``python3`` is never shimmed. A POSIX shim is a symlink to ``cmd_mox/shim.py``
whose shebang is ``#!/usr/bin/env python3``, so a shim named ``python3`` would
resolve its own interpreter back to itself. The clock adapter therefore runs as
the real interpreter by default, and a test that needs a controlled clock
points ``NETSUKE_RELEASE_ADMISSION_CLOCK_ADAPTER`` at a shim it registers under
a name of its own choosing.

*The doubles are spies, not mocks.* Their recorded invocations are the
assertions this suite makes -- the exact argument vector, the exact standard
input, the exact call count, and the calls that must not happen. Each is a
direct comparison against :attr:`Subprocess.calls`, including the empty list
an early-exit case expects, so a reader sees the expectation as data rather
than as a library call whose semantics they must go and look up.

*A spy's own count expectation is not verified.* cmd-mox's ``CountVerifier``
checks the mocks only, so ``times_called`` sets an expectation nothing ever
reads. Every count assertion below is therefore a direct comparison against
:attr:`Subprocess.calls`, and a reader must not mistake it for the library
doing the work. For the same reason this wrapper does not forward
``assert_not_called``: the library's own version says exactly what an empty
:attr:`calls` comparison says, so re-exporting it would add a name and a
frame without adding behaviour.

Replacing a behaviour always goes through :meth:`Subprocess.wraps`, never
``returns``. cmd-mox's ``returns`` rebuilds the response *and* clears the
handler, so a scenario that called it would silently discard the happy path it
means to wrap; ``runs`` sets the handler and leaves the response alone, which
makes latest-wins composition the natural form.
"""

import collections.abc as cabc
import sys
import typing as typ
from pathlib import Path

if typ.TYPE_CHECKING:
    import subprocess

    from cmd_mox import CmdMox
    from cmd_mox.ipc import Invocation
    from cmd_mox.test_doubles import CommandDouble

REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT_PATH = REPO_ROOT / ".github" / "scripts" / "release_admission.py"
PYTHON_PATH = Path(sys.executable)
REVISION = "a" * 40
GITHUB_REPOSITORY = "leynos/netsuke"

#: Adapter variables that name an executable directly, rather than by PATH.
GH_ADAPTER = "NETSUKE_RELEASE_ADMISSION_GH_ADAPTER"
GIT_ADAPTER = "NETSUKE_RELEASE_ADMISSION_GIT_ADAPTER"
CLOCK_ADAPTER = "NETSUKE_RELEASE_ADMISSION_CLOCK_ADAPTER"
METRICS_SINK_VARIABLE = "NETSUKE_RELEASE_ADMISSION_METRICS_SINK"
OUTPUT_SINK_VARIABLE = "NETSUKE_RELEASE_ADMISSION_OUTPUT_SINK"
TRACE_SINK_VARIABLE = "NETSUKE_RELEASE_ADMISSION_TRACE_SINK"

#: The names the six doubled adapters are registered under. They are the six
#: variable names lowercased, so a failure message names the variable.
ADAPTED_GH = "netsuke_release_admission_gh_adapter"
ADAPTED_GIT = "netsuke_release_admission_git_adapter"
ADAPTED_CLOCK = "netsuke_release_admission_clock_adapter"
ADAPTED_METRICS_SINK = "netsuke_release_admission_metrics_sink"
ADAPTED_OUTPUT_SINK = "netsuke_release_admission_output_sink"
ADAPTED_TRACE_SINK = "netsuke_release_admission_trace_sink"

#: The clock's argv, as the gate renders it.
CLOCK_ARGUMENTS: typ.Final[tuple[str, str]] = (
    "-c",
    "import time; print(time.monotonic())",
)

#: A handler answers one invocation with ``(stdout, stderr, exit_code)``.
type Handler = cabc.Callable[[Invocation], tuple[str, str, int]]

#: A wrapper answers one invocation, either itself or through the handler it
#: replaced. Receiving the previous handler is what lets a scenario express
#: "fail instead" and "delay, then behave normally" without either of them
#: restating the happy path.
type Wrapper = cabc.Callable[[Invocation, Handler], tuple[str, str, int]]


class Subprocess:
    """Configure and inspect one cmd-mox double as a subprocess boundary.

    cmd-mox spells a double's configuration as a fluent chain that returns the
    double itself, and records each invocation as an :class:`Invocation`. This
    wrapper keeps both behind one small surface, so a test reads as a statement
    about the boundary rather than about the mocking library.
    """

    def __init__(self, double: CommandDouble) -> None:
        """Wrap one registered double."""
        self._double = double

    @property
    def double(self) -> CommandDouble:
        """The wrapped double, for a caller that must register or spy."""
        return self._double

    @property
    def handler(self) -> Handler:
        """The handler that currently answers this double.

        A double has a handler because :func:`install_boundaries` gave it one,
        and every replacement is composed over that handler rather than
        clearing it, so reading it back is always defined.
        """
        return typ.cast("Handler", self._double.handler)

    def runs(self, handler: Handler) -> Subprocess:
        """Answer every invocation by calling *handler*.

        Returns
        -------
        Subprocess
            This boundary, for chaining.
        """
        self._double.runs(typ.cast("typ.Any", handler))
        return self

    def wraps(self, wrapper: Wrapper) -> Subprocess:
        """Compose *wrapper* over the handler already installed.

        Returns
        -------
        Subprocess
            This boundary, for chaining.
        """
        previous = self.handler
        return self.runs(lambda invocation: wrapper(invocation, previous))

    @property
    def calls(self) -> list[list[str]]:
        """The recorded argument vectors, excluding the program name."""
        return [list(invocation.args) for invocation in self._double.invocations]

    @property
    def stdins(self) -> list[str]:
        """The recorded standard inputs, one per invocation."""
        return [invocation.stdin for invocation in self._double.invocations]

    @property
    def envs(self) -> list[dict[str, str]]:
        """The child environments, one per invocation.

        A shim reports the environment it inherited, which is how a test reads
        the revision the workflow exported without inspecting the command line
        for it.
        """
        return [dict(invocation.env) for invocation in self._double.invocations]


class Boundaries(typ.NamedTuple):
    """Hold the command doubles one gate run reaches by their default names."""

    gh: Subprocess
    git: Subprocess
    clock: Subprocess


class AdaptedBoundaries(typ.NamedTuple):
    """Hold the doubles a run reaches through its adapter variables.

    The gate's adapter variables name an executable directly rather than a
    program to find on ``PATH``, and this is what that contract looks like with
    a registered double in every position: six doubles under names of their
    own, named by the six variables. The values differ from the defaults only
    in being shims, so a test proves the adapter contract itself rather than
    the default resolution.

    The field names are the gate's own variable suffixes, lowercased, so a test
    reads ``adapted.metrics`` for
    ``NETSUKE_RELEASE_ADMISSION_METRICS_SINK``.
    """

    gh: Subprocess
    git: Subprocess
    clock: Subprocess
    metrics: Subprocess
    output: Subprocess
    trace: Subprocess

    def variables(self, cmd_mox: CmdMox) -> dict[str, str]:
        """Return the six adapter variables, each naming one registered shim.

        Returns
        -------
        dict[str, str]
            Environment overrides that redirect every adapter at a double.
        """
        return {
            GH_ADAPTER: shim(cmd_mox, ADAPTED_GH),
            GIT_ADAPTER: shim(cmd_mox, ADAPTED_GIT),
            CLOCK_ADAPTER: shim(cmd_mox, ADAPTED_CLOCK),
            METRICS_SINK_VARIABLE: shim(cmd_mox, ADAPTED_METRICS_SINK),
            OUTPUT_SINK_VARIABLE: shim(cmd_mox, ADAPTED_OUTPUT_SINK),
            TRACE_SINK_VARIABLE: shim(cmd_mox, ADAPTED_TRACE_SINK),
        }


def reply_to_sink(invocation: Invocation) -> tuple[str, str, int]:
    """Append one record to the target the sink adapter was named with.

    A sink adapter's whole contract is ``cat >>"$1"``: its only argument is the
    target path and the record arrives on its standard input. Expressing it as
    a handler rather than as a shell script keeps the sink's own argv and
    standard input as the thing under assertion, instead of burying them in a
    script the test would have to re-read.

    Returns
    -------
    tuple[str, str, int]
        Standard output, standard error, and exit status. A sink named with no
        target refuses, which no correct configuration reaches.
    """
    if not invocation.args:
        return "", "sink adapter was named without a target\n", 1
    with Path(invocation.args[0]).open(
        "a", encoding="utf-8", errors="surrogateescape"
    ) as stream:
        stream.write(invocation.stdin)
    return "", "", 0


def install_adapted(cmd_mox: CmdMox) -> AdaptedBoundaries:
    """Register a double in every position the gate can be redirected through.

    The three command adapters answer their happy paths, and each of the three
    record sinks appends to the target it is named with. A test that wants a
    sink to fail replaces it with :meth:`Subprocess.wraps`.

    Parameters
    ----------
    cmd_mox
        Active controller that owns the shims.

    Returns
    -------
    AdaptedBoundaries
        The six doubles, each answering its happy path.
    """
    return AdaptedBoundaries(
        gh=Subprocess(cmd_mox.spy(ADAPTED_GH)).runs(reply_to_github),
        git=Subprocess(cmd_mox.spy(ADAPTED_GIT)).runs(reply_to_git),
        clock=Subprocess(cmd_mox.spy(ADAPTED_CLOCK)).runs(reply_to_clock),
        metrics=Subprocess(cmd_mox.spy(ADAPTED_METRICS_SINK)).runs(reply_to_sink),
        output=Subprocess(cmd_mox.spy(ADAPTED_OUTPUT_SINK)).runs(reply_to_sink),
        trace=Subprocess(cmd_mox.spy(ADAPTED_TRACE_SINK)).runs(reply_to_sink),
    )


class GateRun(typ.NamedTuple):
    """Hold one gate run's process result and the records it produced."""

    result: subprocess.CompletedProcess[str]
    metrics: list[dict[str, object]]
    traces: list[dict[str, object]]
    outputs: dict[str, str]
    paths: dict[str, Path]


def shim(cmd_mox: CmdMox, name: str) -> str:
    """Return the absolute path of one registered cmd-mox shim.

    Parameters
    ----------
    cmd_mox
        Active controller, whose environment owns the shim directory.
    name
        Shim name, which must already be registered as a double.

    Returns
    -------
    str
        Absolute path of the shim, for an adapter variable that takes a path.

    Notes
    -----
    The controller creates its shim directory when it enters, so an absent one
    means this helper was called outside a live controller. That is a fault in
    the test, which is why it is reported without a ``Raises`` section.
    """
    shim_dir = cmd_mox.environment.shim_dir
    assert shim_dir is not None, "CmdMox must create its command shim directory"
    return str(shim_dir / name)


def install_boundaries(cmd_mox: CmdMox) -> Boundaries:
    """Register the gate's three documented adapters under their default names.

    The GitHub double answers the two admission queries the way the host's own
    ``gh`` would: it echoes ``GITHUB_SHA`` for a commit lookup and names one
    workflow run for a run lookup. The Git double succeeds silently, and the
    clock double prints a number for the monotonic program. A test replaces any
    of those behaviours by calling :meth:`Subprocess.wraps`.

    Parameters
    ----------
    cmd_mox
        Active controller that owns the shims.

    Returns
    -------
    Boundaries
        The three registered doubles, already answering their happy paths.
    """
    return Boundaries(
        gh=Subprocess(cmd_mox.spy("gh")).runs(reply_to_github),
        git=Subprocess(cmd_mox.spy("git")).runs(reply_to_git),
        clock=Subprocess(cmd_mox.spy("clock")).runs(reply_to_clock),
    )


def reply_to_github(invocation: Invocation) -> tuple[str, str, int]:
    """Answer one admission query as the host's own ``gh`` would.

    A commit lookup echoes the revision the workflow supplied. A run lookup
    names one run, because the evidence check admits a revision only when a run
    produced it.

    Returns
    -------
    tuple[str, str, int]
        Standard output, standard error, and exit status.

    Examples
    --------
    The commit lookup answers the argv ``("api", "repos/o/r/commits/<sha>",
    "--jq", ".sha")`` with the child's own ``GITHUB_SHA`` and a newline; any
    other query answers ``1001``.
    """
    query = " ".join(invocation.args)
    if "/commits/" in query:
        return f"{invocation.env.get('GITHUB_SHA', '')}\n", "", 0
    return f"{WORKFLOW_RUN_ID}\n", "", 0


def reply_to_git(invocation: Invocation) -> tuple[str, str, int]:
    """Answer the bounded fetch with silence, as a successful fetch does.

    Returns
    -------
    tuple[str, str, int]
        Standard output, standard error, and exit status.
    """
    del invocation
    return "", "", 0


def reply_to_clock(invocation: Invocation) -> tuple[str, str, int]:
    """Answer the monotonic-clock program with a fixed number.

    Returns
    -------
    tuple[str, str, int]
        Standard output, standard error, and exit status.

    Notes
    -----
    The reading is a constant, so a duration is normally zero. A test that
    needs a measurable duration wraps this handler with one that sleeps, which
    is what the duration test does.
    """
    del invocation
    return "1\n", "", 0


#: The workflow-run identifier the default GitHub double reports.
WORKFLOW_RUN_ID: typ.Final[str] = "1001"
