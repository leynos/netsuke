"""The CodeScene changed-line gate needs a parent commit to compare against.

`coverage-pr-submit.yml` runs on `workflow_run`, so the CodeScene CLI in
its check step has no pull-request context and falls back to the first
parent commit as the base for the changed-line gate. `actions/checkout`
fetches one commit by default, which leaves that parent absent, and the
step dies with `fatal: ambiguous argument 'HEAD~1'`. That failed the
check on every pull request and left `CodeScene Code Coverage (main)`
queued for a submission that never landed, on `main`'s own runs as much
as on any branch.

Nothing else notices: the workflow parses, the job runs, and the failure
names a git revision rather than a checkout depth. So the depth is
pinned here, against every job that runs the check rather than against
the one that runs it today.

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

#: `HEAD` and its first parent. `fetch-depth: 0` means the whole history
#: and is also sufficient; anything between one and this is not.
REQUIRED_FETCH_DEPTH: typ.Final[int] = 2

#: What `actions/checkout` fetches when a step names no `fetch-depth`.
DEFAULT_FETCH_DEPTH: typ.Final[int] = 1

#: `fetch-depth: 0` is the whole history rather than an empty one.
FULL_HISTORY_DEPTH: typ.Final[int] = 0


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
    # step submits the report and compares nothing, so it needs no
    # parent commit and is not held to this depth.
    if _action_of(step) != CODESCENE_COVERAGE_ACTION:
        return False
    with_block = step.get("with")
    return isinstance(with_block, dict) and with_block.get("mode") == "check"


def _declared_depth(step: dict[str, object]) -> int:
    """Return a checkout step's ``fetch-depth``, or the action's default."""
    # A workflow may write the depth as a YAML integer or as a quoted
    # string; both reach the action as the same value, so both are read
    # here. Anything else is not a depth and is reported as the default,
    # which fails the assertion rather than passing unexamined.
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


def _is_sufficient(depth: int) -> bool:
    """Return whether a depth reaches `HEAD~1`."""
    return depth == FULL_HISTORY_DEPTH or depth >= REQUIRED_FETCH_DEPTH


def _checkout_depths(steps: list[dict[str, object]]) -> list[int]:
    """Return the depth each checkout step in a job fetches."""
    return [
        _declared_depth(step) for step in steps if _action_of(step) == CHECKOUT_ACTION
    ]


def _jobs_running_the_check() -> list[tuple[str, str, list[dict[str, object]]]]:
    """Return every job that runs the CodeScene changed-line gate."""
    found: list[tuple[str, str, list[dict[str, object]]]] = []
    for path in _workflow_paths():
        jobs = require_mapping(load_workflow(path).get("jobs"), f"{path.name} jobs")
        for name, declaration in jobs.items():
            job = require_mapping(declaration, f"{path.name}:{name}")
            steps = [
                step
                for step in require_list(job.get("steps", []), f"{name} steps")
                if isinstance(step, dict)
            ]
            if any(_runs_the_codescene_check(step) for step in steps):
                found.append((path.name, str(name), steps))
    return found


def test_the_codescene_check_is_still_run_somewhere() -> None:
    """The depth assertion below is vacuous if nothing runs the gate.

    A rename of the shared action, or of its `check` mode, would leave
    the assertion passing over an empty set while the gate it protects
    ran unguarded.
    """
    assert _jobs_running_the_check(), (
        f"no job runs {CODESCENE_COVERAGE_ACTION} in check mode; either the "
        f"gate was removed deliberately, in which case delete this contract, "
        f"or the action reference drifted and the depth is now unguarded"
    )


def test_every_codescene_check_job_fetches_its_parent_commit() -> None:
    """Without `HEAD~1` the gate fails on a git revision, not on coverage.

    The CLI runs outside pull-request context here and takes the first
    parent as its base. At the default depth of one that commit is not
    in the clone, so the step exits non-zero with `fatal: ambiguous
    argument 'HEAD~1'` and the coverage check waits for a submission
    that never arrives.

    Proved by mutation: setting the checkout back to `fetch-depth: 1`,
    or deleting the key, fails this test and nothing else.
    """
    shallow = {
        f"{workflow}:{job}": depths
        for workflow, job, steps in _jobs_running_the_check()
        if not all(map(_is_sufficient, (depths := _checkout_depths(steps))))
    }
    assert not shallow, (
        f"these jobs run the CodeScene changed-line gate over a checkout that "
        f"cannot reach HEAD~1, as job to declared depths: {shallow}; "
        f"fetch-depth must be at least {REQUIRED_FETCH_DEPTH}, or 0 for the "
        f"whole history"
    )


def test_a_job_running_the_check_checks_out_at_all() -> None:
    """A gate with no checkout has no history to read, shallow or not.

    Stated apart from the depth assertion because a job whose checkout
    step was deleted declares no depths at all, and `all` over an empty
    list is true.
    """
    without = [
        f"{workflow}:{job}"
        for workflow, job, steps in _jobs_running_the_check()
        if not _checkout_depths(steps)
    ]
    assert not without, (
        f"these jobs run the CodeScene changed-line gate without checking the "
        f"repository out: {without}; the CLI reads its base from git history"
    )
