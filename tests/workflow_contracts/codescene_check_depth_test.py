"""The CodeScene changed-line gate reads git history, so it needs one.

`upload-codescene-coverage` documents `mode: check` as diffing against a
merge base and requiring a `fetch-depth: 0` checkout. `actions/checkout`
fetches one commit by default, which leaves the CLI with no history at
all: it dies with `fatal: ambiguous argument 'HEAD~1': unknown revision
or path not in the working tree`. That failed the check on every pull
request and left `CodeScene Code Coverage (main)` waiting for a
submission that never landed, on `main`'s own runs as much as on any
branch.

Nothing else notices: the workflow parses, the job runs, and the failure
names a git revision rather than a checkout depth. So the depth is
pinned here, against every job that runs the gate rather than the one
that runs it today, and the checkout is required to precede the gate,
because a checkout after it arrives too late to be read.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

from workflow_loading import (
    REPO_ROOT,
    load_workflow,
    require_list,
    require_mapping,
)

if typ.TYPE_CHECKING:
    from pathlib import Path

#: The shared action whose `check` mode runs the CodeScene CLI.
CODESCENE_COVERAGE_ACTION: typ.Final[str] = (
    "leynos/shared-actions/.github/actions/upload-codescene-coverage"
)

#: The action that provides the checkout the CLI reads its history from.
CHECKOUT_ACTION: typ.Final[str] = "actions/checkout"

#: `fetch-depth: 0` is the whole history rather than an empty one, and is
#: what the shared action documents `mode: check` as requiring.
FULL_HISTORY_DEPTH: typ.Final[int] = 0

#: What `actions/checkout` fetches when a step names no `fetch-depth`.
DEFAULT_FETCH_DEPTH: typ.Final[int] = 1


class CheckJob(typ.NamedTuple):
    """One job that runs the CodeScene changed-line gate.

    Attributes
    ----------
    workflow : str
        The workflow file's name.
    job : str
        The job's identifier.
    steps : list[dict[str, object]]
        The job's steps, in declaration order.
    """

    workflow: str
    job: str
    steps: list[dict[str, object]]

    def __str__(self) -> str:
        """Return a location suitable for a failure message."""
        return f"{self.workflow}:{self.job}"


def _workflow_paths() -> list[Path]:
    """Return every workflow file, in file-name order."""
    # Both extensions, because GitHub accepts either and a job moved to
    # the other one would escape this contract without failing it.
    directory = REPO_ROOT / ".github" / "workflows"
    assert directory.is_dir(), f"{directory} must be the workflow directory"
    return sorted(
        path for pattern in ("*.yml", "*.yaml") for path in directory.glob(pattern)
    )


def _action_of(step: dict[str, object]) -> str:
    """Return a step's action reference without its version, or the empty string."""
    # Split on the version separator rather than matching a prefix: a
    # prefix match would accept `upload-codescene-coverage-legacy`, and a
    # substring match would accept an action merely mentioning the name.
    uses = step.get("uses")
    return uses.split("@", 1)[0] if isinstance(uses, str) else ""


def _runs_the_codescene_check(step: dict[str, object]) -> bool:
    """Return whether a step runs the CodeScene CLI's changed-line gate."""
    # `mode: check` is the mode that reads git history. An upload-only
    # step submits the report and diffs nothing, so it needs no history
    # and is not held to this depth.
    if _action_of(step) != CODESCENE_COVERAGE_ACTION:
        return False
    with_block = step.get("with")
    return isinstance(with_block, dict) and with_block.get("mode") == "check"


def _declared_depth(step: dict[str, object]) -> int:
    """Return a checkout step's ``fetch-depth``, or the action's default."""
    # A workflow may write the depth as a YAML integer or as a quoted
    # string; both reach the action as the same value, so both are read
    # here. Anything else is not a depth and reads as the default, which
    # fails the assertion rather than passing unexamined.
    with_block = step.get("with")
    if not isinstance(with_block, dict) or "fetch-depth" not in with_block:
        return DEFAULT_FETCH_DEPTH
    match with_block["fetch-depth"]:
        case bool():
            # `True` is an `int` in Python and is not a depth.
            return DEFAULT_FETCH_DEPTH
        case int() as depth:
            return depth
        case str() as text if text.strip().lstrip("-").isdigit():
            return int(text.strip())
        case _:
            return DEFAULT_FETCH_DEPTH


def _fetches_whole_history(step: dict[str, object]) -> bool:
    """Return whether a step is a checkout fetching the whole history."""
    return (
        _action_of(step) == CHECKOUT_ACTION
        and _declared_depth(step) == FULL_HISTORY_DEPTH
    )


def _job_steps(job: dict[str, object], description: str) -> list[dict[str, object]]:
    """Return a job's steps, dropping anything that is not a mapping."""
    return [
        step
        for step in require_list(job.get("steps", []), f"{description} steps")
        if isinstance(step, dict)
    ]


def _check_jobs() -> list[CheckJob]:
    """Return every job that runs the CodeScene changed-line gate."""
    found: list[CheckJob] = []
    for path in _workflow_paths():
        jobs = require_mapping(load_workflow(path).get("jobs"), f"{path.name} jobs")
        for name, declaration in jobs.items():
            description = f"{path.name}:{name}"
            steps = _job_steps(require_mapping(declaration, description), description)
            if any(map(_runs_the_codescene_check, steps)):
                found.append(CheckJob(path.name, str(name), steps))
    return found


def _gate_is_prepared(job: CheckJob) -> bool:
    """Return whether a full-history checkout precedes the job's first gate."""
    # Both indices are taken from the same step list, so comparing them
    # is what states the ordering. A checkout after the gate leaves the
    # same empty history when the CLI runs as no checkout at all.
    gate = next(
        (
            index
            for index, step in enumerate(job.steps)
            if _runs_the_codescene_check(step)
        ),
        None,
    )
    checkout = next(
        (index for index, step in enumerate(job.steps) if _fetches_whole_history(step)),
        None,
    )
    return gate is not None and checkout is not None and checkout < gate


def test_the_codescene_check_is_still_run_somewhere() -> None:
    """The depth assertion below is vacuous if nothing runs the gate.

    A rename of the shared action, or of its `check` mode, would leave
    the assertion passing over an empty set while the gate it protects
    ran unguarded.
    """
    assert _check_jobs(), (
        f"no job runs {CODESCENE_COVERAGE_ACTION} in check mode; either the "
        f"gate was removed deliberately, in which case delete this contract, "
        f"or the action reference drifted and the depth is now unguarded"
    )


def test_a_whole_history_checkout_precedes_every_codescene_check() -> None:
    """Without history the gate fails on a git revision, not on coverage.

    The shared action documents `mode: check` as diffing against a merge
    base and requiring `fetch-depth: 0`. At the default depth of one the
    step exits non-zero with `fatal: ambiguous argument 'HEAD~1'`, and
    the coverage check then waits for a submission that never arrives.

    Order is part of the requirement rather than a separate one. A
    checkout placed after the gate leaves the same empty history when
    the CLI runs, so a contract looking anywhere in the job would accept
    a job that fails exactly as before.

    Proved by mutation: `fetch-depth: 2`, `fetch-depth: 1`, deleting the
    key, deleting the checkout step, and moving the checkout after the
    gate each fail this test.
    """
    unprepared = [str(job) for job in _check_jobs() if not _gate_is_prepared(job)]
    assert not unprepared, (
        f"these jobs run the CodeScene changed-line gate without a "
        f"fetch-depth: {FULL_HISTORY_DEPTH} checkout before it: {unprepared}; "
        f"the CLI diffs against a merge base and reads that from git history"
    )
