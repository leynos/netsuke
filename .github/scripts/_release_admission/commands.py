"""Run the gate's external effects: API requests, fetches, clock, and sinks.

Nothing here decides anything. Every function performs one bounded effect and
returns what the operating system said, so the policy stays a pure function of
observed values and the tests substitute one seam rather than a whole script.

Programs are reached through cuprum, as the estate's scripting standard
requires. The catalogues are built from the *configured* adapter names rather
than a fixed list, because the adapter environment variables are a documented
part of this gate's contract: an operator may point the gate at a wrapper, and
the catalogue is what records which executables that configuration permits.

Bounding is :mod:`bounding`'s, and the reason it is a module of its own is that
it needs more than cuprum offers: a bound that reaches a command's descendants.
The shell had that through GNU ``timeout``, which signals the command's process
group; see that module for the mechanism, the stream wiring, and the trade-off.

Only an adapter's *stdout* is private. The shell captured it in a command
substitution, so nothing read it; its stderr was never redirected and reached
the runner log. A command whose output is only reported rather than parsed --
the ``git`` fetch -- gets both streams through, and a launch the operating
system refuses is reported with the status the shell produced for the same
refusal.

What each command *did* is reported as a :class:`policy.CommandResult` rather
than as a shell status, so this module never interprets a number. The
translation from cuprum's result and the operating system's errors belongs to
:mod:`statuses`, which both this module and :mod:`delivery` call.

Sink delivery has no cuprum equivalent and lives in :mod:`delivery`.
"""

import dataclasses
import typing as typ

from cuprum import (
    Program,
    ProgramCatalogue,
    ProjectSettings,
    SafeCmd,
)
from cuprum import sh as cuprum_sh

from . import environment, policy, statuses
from .bounding import run_bounded_sync, run_unbounded_sync

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: Environment variables naming the executables the gate runs.
GH_ADAPTER_VARIABLE = "NETSUKE_RELEASE_ADMISSION_GH_ADAPTER"
GIT_ADAPTER_VARIABLE = "NETSUKE_RELEASE_ADMISSION_GIT_ADAPTER"
CLOCK_ADAPTER_VARIABLE = "NETSUKE_RELEASE_ADMISSION_CLOCK_ADAPTER"

#: Environment variables naming the record sinks. Empty selects the in-process
#: append, which is why these are read as raw strings rather than through a
#: defaulting helper.
METRICS_SINK_VARIABLE = "NETSUKE_RELEASE_ADMISSION_METRICS_SINK"
OUTPUT_SINK_VARIABLE = "NETSUKE_RELEASE_ADMISSION_OUTPUT_SINK"
TRACE_SINK_VARIABLE = "NETSUKE_RELEASE_ADMISSION_TRACE_SINK"

#: Documented adapter defaults, as the shell resolved them.
DEFAULT_GH_ADAPTER = "gh"
DEFAULT_GIT_ADAPTER = "git"
DEFAULT_CLOCK_ADAPTER = "python3"

#: The clock adapter is asked for the monotonic clock through its own ``-c``.
CLOCK_ARGUMENTS: typ.Final[tuple[str, ...]] = (
    "-c",
    "import time; print(time.monotonic())",
)


@dataclasses.dataclass(frozen=True, slots=True)
class Programs:
    """Hold the distinct executables one gate run is configured to run.

    The three adapters may name the same file -- the tests routinely point two
    of them at one script -- so the catalogue is built from the de-duplicated
    set. Cuprum refuses a program owned by two projects, and a duplicate name
    is a configuration the gate has always accepted.
    """

    gh: str
    git: str
    clock: str

    @classmethod
    def from_environment(cls, variables: cabc.Mapping[str, str]) -> Programs:
        """Read the three adapter names, defaulting unset *and* empty values.

        An empty value substitutes the default exactly as an unset one does,
        because that is Bash's ``${VAR:-default}`` form -- the one
        :func:`~._release_admission.environment.optional` implements, and the
        one the shell read every adapter through. A configuration that exported
        an empty adapter therefore ran ``gh``, ``git``, or ``python3``, and
        that is what is reproduced here rather than letting the empty name
        reach the operating system as a program.

        Returns
        -------
        Programs
            The three resolved adapter names.
        """
        return cls(
            gh=environment.optional(variables, GH_ADAPTER_VARIABLE, DEFAULT_GH_ADAPTER),
            git=environment.optional(
                variables, GIT_ADAPTER_VARIABLE, DEFAULT_GIT_ADAPTER
            ),
            clock=environment.optional(
                variables, CLOCK_ADAPTER_VARIABLE, DEFAULT_CLOCK_ADAPTER
            ),
        )

    def catalogue(self) -> ProgramCatalogue:
        """Return a catalogue permitting exactly these three executables.

        Returns
        -------
        ProgramCatalogue
            One project owning the de-duplicated adapter programs.
        """
        distinct = tuple(dict.fromkeys(Program(name) for name in self.names()))
        return ProgramCatalogue(
            projects=[
                ProjectSettings(
                    name="release-admission",
                    programs=distinct,
                    documentation_locations=(
                        "https://docs.github.com/en/rest",
                        "https://git-scm.com/docs",
                    ),
                    noise_rules=(),
                )
            ]
        )

    def names(self) -> tuple[str, ...]:
        """Return the adapter names in a stable order."""
        return (self.gh, self.git, self.clock)


def github_resolve_commit(
    programs: Programs, *, repository: str, revision: str, timeout_seconds: int
) -> policy.CommandResult:
    """Ask the GitHub API which commit a revision resolves to.

    Returns
    -------
    policy.CommandResult
        The outcome and status, plus the reported SHA with its trailing
        newline removed. The output is empty whenever the command did not
        succeed, because a failed command substitution leaves Bash's variable
        empty.
    """
    catalogue = programs.catalogue()
    command = cuprum_sh.make(Program(programs.gh), catalogue=catalogue)(
        "api", f"repos/{repository}/commits/{revision}", "--jq", ".sha"
    )
    return _status_and_output(
        command, program=programs.gh, timeout_seconds=timeout_seconds
    )


def github_find_workflow_run(
    programs: Programs, *, repository: str, revision: str, timeout_seconds: int
) -> policy.CommandResult:
    """Ask the GitHub API for the workflow run covering a revision.

    The ``// empty`` filter turns a revision with no matching run into an empty
    line rather than the literal ``null``, and an empty run identifier is what
    :func:`policy.classify_evidence` refuses.

    Returns
    -------
    policy.CommandResult
        The outcome and status, plus the run identifier, empty when the
        command failed.
    """
    catalogue = programs.catalogue()
    query = f"repos/{repository}/actions/runs?head_sha={revision}&per_page=1"
    command = cuprum_sh.make(Program(programs.gh), catalogue=catalogue)(
        "api", query, "--jq", ".workflow_runs[0].id // empty"
    )
    return _status_and_output(
        command, program=programs.gh, timeout_seconds=timeout_seconds
    )


def git_fetch_revision(
    programs: Programs, *, revision: str, timeout_seconds: int
) -> policy.CommandResult:
    """Fetch one revision from ``origin``, without tags and only one deep.

    Both of the fetch's streams are inherited rather than captured. The shell
    never redirected either one, so ``git``'s progress on stderr and its
    report on stdout both reached the runner log; capturing them would have
    discarded the only diagnostic a failed fetch produces.

    Returns
    -------
    policy.CommandResult
        The outcome and the command's status in Bash's convention, the timeout
        status the bound uses, or the launch status GNU ``timeout`` reported
        for a ``git`` it could not run. The fetch is reported rather than
        parsed, so its output is never retained.
    """
    catalogue = programs.catalogue()
    command = cuprum_sh.make(Program(programs.git), catalogue=catalogue)(
        "fetch", "--depth", "1", "--no-tags", "origin", "--", revision
    )
    try:
        return statuses.translate_result(
            run_bounded_sync(
                command, timeout_seconds=timeout_seconds, inherit_streams=True
            )
        )
    except OSError as error:
        statuses.report_launch_failure(programs.git, error)
        return statuses.launch_failure_result(error)


def read_monotonic_seconds(programs: Programs) -> policy.CommandResult:
    """Read the clock adapter once, unbounded as the shell read it.

    Returns
    -------
    policy.CommandResult
        The outcome and status, plus the reading, empty when the command
        failed.
    """
    catalogue = programs.catalogue()
    command = cuprum_sh.make(Program(programs.clock), catalogue=catalogue)(
        *CLOCK_ARGUMENTS
    )
    return _status_and_output(command, program=programs.clock, timeout_seconds=None)


def _status_and_output(
    command: SafeCmd, *, program: str, timeout_seconds: int | None
) -> policy.CommandResult:
    """Run a command whose standard output the gate parses, and return both.

    The command is the shell's own command substitution: its standard output is
    captured in a pipe the gate reads to end-of-file, so a caller never parses a
    prefix of it, and its standard error is inherited so the adapter's own
    diagnostic reaches the runner log as it did before. Passing ``None`` for
    *timeout_seconds* is the clock adapter, which the shell read with no
    ``timeout`` around it at all.

    A command the operating system refused to launch is reported with the
    status GNU ``timeout`` produced for the same refusal rather than allowed to
    escape as an exception.

    Returns
    -------
    policy.CommandResult
        The outcome and status in Bash's convention, and the retained standard
        output with its trailing newlines removed. The output is empty whenever
        the command did not succeed, because a failed command substitution left
        Bash's variable empty.
    """
    try:
        result = (
            run_unbounded_sync(command)
            if timeout_seconds is None
            else run_bounded_sync(command, timeout_seconds=timeout_seconds)
        )
    except OSError as error:
        statuses.report_launch_failure(program, error)
        return statuses.launch_failure_result(error)
    return statuses.translate_result(result)


def render_duration(started: str, finished: str) -> str:
    """Render the seconds between two clock readings, or ``""``.

    The shell computed this with a ``python3`` helper that parsed both readings
    with ``float`` and printed ``max(0.0, finished - started)``, so ``nan`` and
    ``inf`` -- which ``float`` accepts -- behave as they did. A reading that
    would not parse left the command substitution empty, which is why a garbage
    clock costs the duration record rather than the operation's result.

    Returns
    -------
    str
        The rendered duration, or the empty string when either reading is not a
        number.

    Examples
    --------
    >>> render_duration("1.0", "2.5")
    '1.5'
    >>> render_duration("1.0", "not-a-number")
    ''
    """
    try:
        started_seconds = float(started)
        finished_seconds = float(finished)
    except ValueError:
        return ""
    return str(max(0.0, finished_seconds - started_seconds))
