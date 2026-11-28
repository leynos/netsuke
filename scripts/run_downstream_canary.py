#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# dependencies = ["cuprum>=0.1.0,<0.2.0"]
# ///
"""Drive one downstream migration canary and record its provenance.

The ``downstream-canary`` composite action calls this script in three steps so
each phase is visible in the job log:

``generate``
    Run the candidate's ``netsuke --verbose generate --output build.ninja`` in
    the downstream checkout.
``run``
    Refuse a generated manifest that contains a pattern the selected lane must
    not reach, then run each requested target with ``ninja -f build.ninja``.
``report``
    Always runs. Write the bounded provenance record and one job-summary line
    from whatever the earlier steps recorded.

The steps share a small JSON state file. Statuses are drawn from a closed
vocabulary and command output stays in the job log, so the provenance record
never carries unbounded text.

The decisions are pure functions of their inputs: lane isolation and the
``run`` step's exit status in ``downstream_canary_isolation``, and the record
and summary a report writes in :func:`compose_report`. The step handlers are
the edges that read files, launch tools, and print. They receive the
environment and the executable lookup as a :class:`Host` built once in
:func:`main`; every process launch goes through ``downstream_canary_process``,
and the shared state through ``downstream_canary_state``.
"""

import functools
import json
import re
import sys
import typing as typ

from downstream_canary_arguments import build_parser, normalize_lists
from downstream_canary_isolation import judge_isolation, run_status, target_statuses
from downstream_canary_process import Host, observed_head, run_tool
from downstream_canary_provenance import (
    FAILED,
    NOT_RUN,
    PASSED,
    CanaryIdentity,
    provenance_record,
    summary_line,
)
from downstream_canary_state import save_state, state_or_nothing_run

if typ.TYPE_CHECKING:
    import argparse
    import collections.abc as cabc

    from downstream_canary_process import Which

#: The generated manifest, relative to the downstream checkout.
MANIFEST = "build.ninja"
#: A lane selector's variable name.
#: ``\Z`` rather than ``$``, which would also accept a trailing newline.
SELECTOR_NAME = re.compile(r"^[A-Z_][A-Z0-9_]*\Z")


def parse_assignments(pairs: cabc.Iterable[str]) -> dict[str, str]:
    """Parse ``NAME=value`` environment assignments.

    Parameters
    ----------
    pairs
        Assignments, such as the lane selector ``MXD_BACKEND=postgres``.

    Returns
    -------
    dict[str, str]
        The values by variable name.

    Raises
    ------
    ValueError
        When an assignment has no ``=`` or an invalid variable name.

    Examples
    --------
    >>> parse_assignments(["MXD_BACKEND=sqlite"])
    {'MXD_BACKEND': 'sqlite'}
    """
    selectors: dict[str, str] = {}
    for position, pair in enumerate(pairs, start=1):
        name, separator, value = pair.partition("=")
        if not separator or SELECTOR_NAME.match(name) is None:
            # Name the position only: an assignment may carry a credential.
            msg = f"environment assignment {position} is not NAME=value"
            raise ValueError(msg)
        selectors[name] = value
    return selectors


def child_environment(
    arguments: argparse.Namespace, inherited: cabc.Mapping[str, str]
) -> dict[str, str]:
    """Return the inherited environment with the canary's assignments applied.

    Lane selectors are recorded in the provenance; the extra environment, such
    as a service connection string, reaches the tools but is never recorded.
    Selectors are applied last, so they win over both.

    Parameters
    ----------
    arguments
        Parsed ``generate`` or ``run`` arguments.
    inherited
        The environment the step itself received.

    Returns
    -------
    dict[str, str]
        The child process environment.
    """
    return {
        **inherited,
        **parse_assignments(arguments.environment),
        **parse_assignments(arguments.selector),
    }


def resolve_ninja(env: cabc.Mapping[str, str], which: Which) -> str:
    """Return the Ninja executable the child environment selects.

    Ninja is looked up on the ``PATH`` it will run with, so an
    ``--environment`` override selects the executable as well as reaching it.

    Parameters
    ----------
    env
        The child environment the tool will run with.
    which
        The executable lookup.

    Returns
    -------
    str
        The resolved path, or bare ``ninja`` when the lookup finds none.

    Examples
    --------
    >>> resolve_ninja({"PATH": "/opt"}, lambda name, *, path=None: f"{path}/{name}")
    '/opt/ninja'
    >>> resolve_ninja({}, lambda name, *, path=None: None)
    'ninja'
    """
    return which("ninja", path=env.get("PATH")) or "ninja"


class Report(typ.NamedTuple):
    """What a ``report`` step writes: the record and its job-summary line."""

    record: dict[str, object]
    summary: str


def compose_report(
    identity: CanaryIdentity,
    state: cabc.Mapping[str, object],
    downstream_head: str | None,
) -> Report:
    """Build the provenance record and summary line from the step's inputs.

    Parameters
    ----------
    identity
        The run's identity.
    state
        The shared step state, or ``{}`` when it was unusable.
    downstream_head
        The checkout's observed ``HEAD``, or ``None``.

    Returns
    -------
    Report
        The record and its summary line.
    """
    record = provenance_record(identity, state, downstream_head)
    return Report(record, summary_line(identity, record))


def generate(arguments: argparse.Namespace, *, host: Host) -> int:
    """Generate the downstream Ninja manifest with the candidate.

    Parameters
    ----------
    arguments
        Parsed ``generate`` arguments.
    host
        The step's environment and executable lookup.

    Returns
    -------
    int
        ``0`` when generation succeeds, otherwise ``1``.
    """
    env = child_environment(arguments, host.environment)
    command = [arguments.netsuke, "--verbose", "generate", "--output", MANIFEST]
    passed = run_tool(command, arguments.workdir, env)
    save_state(arguments.state, {"generate": PASSED if passed else FAILED})
    return 0 if passed else 1


def run_targets(arguments: argparse.Namespace, *, host: Host) -> int:
    """Check lane isolation, then run every requested target.

    A manifest that reaches another lane runs nothing, since its targets would
    exercise the wrong lane. Otherwise every target runs even after a failure,
    so the provenance record says which targets failed rather than only the
    first.

    Parameters
    ----------
    arguments
        Parsed ``run`` arguments.
    host
        The step's environment and executable lookup.

    Returns
    -------
    int
        ``0`` when isolation holds and every target passes, otherwise ``1``.
    """
    state = state_or_nothing_run(arguments.state)
    manifest = (arguments.workdir / MANIFEST).read_text(encoding="utf-8")
    isolation = judge_isolation(manifest, arguments.forbid)
    for pattern in isolation.found:
        print(f"{MANIFEST} contains another lane's {pattern!r}", file=sys.stderr)
    statuses = (
        run_each_target(arguments, host=host)
        if isolation.holds
        else dict.fromkeys(arguments.target, NOT_RUN)
    )
    save_state(
        arguments.state, {**state, "isolation": isolation.status, "targets": statuses}
    )
    return run_status(isolation, statuses)


def run_each_target(arguments: argparse.Namespace, *, host: Host) -> dict[str, str]:
    """Run each requested target through Ninja and record its status.

    Parameters
    ----------
    arguments
        Parsed ``run`` arguments.
    host
        The step's environment and executable lookup.

    Returns
    -------
    dict[str, str]
        Each target's ``passed`` or ``failed`` status, in request order. A
        target that failed on any of its runs is ``failed``.
    """
    env = child_environment(arguments, host.environment)
    ninja = resolve_ninja(env, host.which)
    runs = [
        (target, run_tool([ninja, "-f", MANIFEST, target], arguments.workdir, env))
        for target in arguments.target
    ]
    return target_statuses(runs)


def report(arguments: argparse.Namespace, *, host: Host) -> int:
    """Write the provenance record and the job-summary line.

    Parameters
    ----------
    arguments
        Parsed ``report`` arguments.
    host
        The step's environment and executable lookup.

    Returns
    -------
    int
        Always ``0``: reporting never masks the gate's own result.
    """
    identity = CanaryIdentity(
        canary=arguments.canary,
        repository=arguments.repository,
        downstream_revision=arguments.downstream_revision,
        netsuke_revision=arguments.netsuke_revision,
        netsuke_version=arguments.netsuke_version,
        platform=arguments.platform,
        selectors=parse_assignments(arguments.selector),
        targets=tuple(arguments.target),
    )
    git = host.which("git", path=host.environment.get("PATH"))
    written = compose_report(
        identity,
        state_or_nothing_run(arguments.state),
        observed_head(arguments.workdir, git),
    )
    arguments.provenance.write_text(
        json.dumps(written.record, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    if arguments.summary is not None:
        with arguments.summary.open("a", encoding="utf-8") as summary:
            summary.write(written.summary)
    print(json.dumps(written.record, sort_keys=True))
    return 0


def main(argv: cabc.Sequence[str] | None = None, host: Host | None = None) -> int:
    """Run one canary step.

    Parameters
    ----------
    argv
        Arguments excluding the program name, or ``None`` for ``sys.argv``.
    host
        The environment and executable lookup, or ``None`` for this process's
        own; the only place the ambient host is read.

    Returns
    -------
    int
        The step's exit status.
    """
    resolved = Host.ambient() if host is None else host
    handlers = {
        name: functools.partial(handler, host=resolved)
        for name, handler in (
            ("generate", generate),
            ("run", run_targets),
            ("report", report),
        )
    }
    arguments = build_parser(handlers).parse_args(argv)
    try:
        normalize_lists(arguments)
        return arguments.handler(arguments)
    except ValueError as error:
        print(f"downstream canary {arguments.command} failed: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
