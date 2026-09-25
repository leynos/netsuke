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
"""

import json
import os
import re
import shutil
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the canary runs fixed downstream tools.
import sys
import typing as typ

from downstream_canary_arguments import build_parser, normalise_lists
from downstream_canary_provenance import (
    FAILED,
    NOT_RUN,
    PASSED,
    CanaryIdentity,
    provenance_record,
    summary_line,
)

if typ.TYPE_CHECKING:
    import argparse
    import collections.abc as cabc
    from pathlib import Path

#: The generated manifest, relative to the downstream checkout.
MANIFEST = "build.ninja"
#: A lane selector's variable name.
SELECTOR_NAME = re.compile(r"^[A-Z_][A-Z0-9_]*$")


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


def forbidden_matches(manifest: str, patterns: cabc.Iterable[str]) -> list[str]:
    r"""Return the patterns that occur in a generated Ninja manifest.

    A lane's non-interference contract is that no other lane's commands
    survive manifest-time expansion, so a match is a canary failure.

    Parameters
    ----------
    manifest
        The generated manifest's text.
    patterns
        Literal substrings the manifest must not contain.

    Returns
    -------
    list[str]
        The patterns found, in the order given.

    Examples
    --------
    >>> forbidden_matches("build test: phony\n", ["postgres", "test"])
    ['test']
    """
    return [pattern for pattern in patterns if pattern in manifest]


def load_state(path: Path) -> dict[str, object]:
    """Read the shared step state, or an empty state when none was written.

    Parameters
    ----------
    path
        The state file.

    Returns
    -------
    dict[str, object]
        The recorded state.
    """
    if not path.exists():
        return {}
    return json.loads(path.read_text(encoding="utf-8"))


def save_state(path: Path, state: cabc.Mapping[str, object]) -> None:
    """Write the shared step state.

    Parameters
    ----------
    path
        The state file.
    state
        The state to record.
    """
    path.write_text(json.dumps(state, sort_keys=True), encoding="utf-8")


def run_tool(command: cabc.Sequence[str], workdir: Path, env: dict[str, str]) -> bool:
    """Run one tool in the downstream checkout, streaming its output.

    Parameters
    ----------
    command
        The program and its arguments.
    workdir
        The downstream checkout.
    env
        The complete child environment.

    Returns
    -------
    bool
        ``True`` when the tool exits successfully.
    """
    print(f"::group::{' '.join(command)}", flush=True)
    try:
        completed = subprocess.run(command, cwd=workdir, env=env, check=False)  # ruff: ignore[subprocess-without-shell-equals-true] - argument vector, no shell.
    except OSError as error:
        print(f"cannot start {command[0]}: {error}", file=sys.stderr)
        return False
    finally:
        print("::endgroup::", flush=True)
    return completed.returncode == 0


def child_environment(arguments: argparse.Namespace) -> dict[str, str]:
    """Return the inherited environment with the canary's assignments applied.

    Lane selectors are recorded in the provenance; the extra environment, such
    as a service connection string, reaches the tools but is never recorded.

    Parameters
    ----------
    arguments
        Parsed ``generate`` or ``run`` arguments.

    Returns
    -------
    dict[str, str]
        The child process environment.
    """
    return {
        **os.environ,
        **parse_assignments(arguments.environment),
        **parse_assignments(arguments.selector),
    }


def generate(arguments: argparse.Namespace) -> int:
    """Generate the downstream Ninja manifest with the candidate.

    Parameters
    ----------
    arguments
        Parsed ``generate`` arguments.

    Returns
    -------
    int
        ``0`` when generation succeeds, otherwise ``1``.
    """
    env = child_environment(arguments)
    command = [arguments.netsuke, "--verbose", "generate", "--output", MANIFEST]
    passed = run_tool(command, arguments.workdir, env)
    save_state(arguments.state, {"generate": PASSED if passed else FAILED})
    return 0 if passed else 1


def run_targets(arguments: argparse.Namespace) -> int:
    """Check lane isolation, then run every requested target.

    A manifest that reaches another lane runs nothing, since its targets would
    exercise the wrong lane. Otherwise every target runs even after a failure,
    so the provenance record says which targets failed rather than only the
    first.

    Parameters
    ----------
    arguments
        Parsed ``run`` arguments.

    Returns
    -------
    int
        ``0`` when isolation holds and every target passes, otherwise ``1``.
    """
    state = load_state(arguments.state)
    manifest = (arguments.workdir / MANIFEST).read_text(encoding="utf-8")
    found = forbidden_matches(manifest, arguments.forbid)
    for pattern in found:
        print(f"{MANIFEST} contains another lane's {pattern!r}", file=sys.stderr)
    state["isolation"] = FAILED if found else PASSED
    statuses = (
        dict.fromkeys(arguments.target, NOT_RUN)
        if found
        else run_each_target(arguments)
    )
    state["targets"] = statuses
    save_state(arguments.state, state)
    return 1 if found or FAILED in statuses.values() else 0


def run_each_target(arguments: argparse.Namespace) -> dict[str, str]:
    """Run each requested target through Ninja and record its status.

    Parameters
    ----------
    arguments
        Parsed ``run`` arguments.

    Returns
    -------
    dict[str, str]
        Each target's ``passed`` or ``failed`` status, in request order.
    """
    ninja = shutil.which("ninja") or "ninja"
    env = child_environment(arguments)
    return {
        target: PASSED
        if run_tool([ninja, "-f", MANIFEST, target], arguments.workdir, env)
        else FAILED
        for target in arguments.target
    }


def observed_head(workdir: Path) -> str | None:
    """Return the downstream checkout's ``HEAD`` commit, if Git can read it.

    Parameters
    ----------
    workdir
        The downstream checkout.

    Returns
    -------
    str | None
        The full commit identifier, or ``None`` when the checkout is missing.
    """
    git = shutil.which("git")
    if git is None or not workdir.is_dir():
        return None
    completed = subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - resolved program, list arguments.
        [git, "-C", str(workdir), "rev-parse", "HEAD"],
        check=False,
        capture_output=True,
        text=True,
    )
    return completed.stdout.strip() if completed.returncode == 0 else None


def report(arguments: argparse.Namespace) -> int:
    """Write the provenance record and the job-summary line.

    Parameters
    ----------
    arguments
        Parsed ``report`` arguments.

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
    record = provenance_record(
        identity, load_state(arguments.state), observed_head(arguments.workdir)
    )
    arguments.provenance.write_text(
        json.dumps(record, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    if arguments.summary is not None:
        with arguments.summary.open("a", encoding="utf-8") as summary:
            summary.write(summary_line(identity, record))
    print(json.dumps(record, sort_keys=True))
    return 0


def main(argv: cabc.Sequence[str] | None = None) -> int:
    """Run one canary step.

    Parameters
    ----------
    argv
        Arguments excluding the program name, or ``None`` for ``sys.argv``.

    Returns
    -------
    int
        The step's exit status.
    """
    handlers = {"generate": generate, "run": run_targets, "report": report}
    arguments = build_parser(handlers).parse_args(argv)
    try:
        normalise_lists(arguments)
        return arguments.handler(arguments)
    except ValueError as error:
        print(f"downstream canary {arguments.command} failed: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
