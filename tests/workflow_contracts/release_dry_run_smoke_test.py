"""The release dry run skips only the Windows smoke its pull request already ran.

`release-dry-run.yml` rehearses the release on every pull request by calling
`release.yml` with `dry-run: true`. Every job on that path builds or packages,
except `windows-native-recipe-smoke`, which builds the debug binary and runs
the native-recipe smoke. The same pull request's `ci.yml` already runs that
exact build and smoke in `build-test-windows`. So the dry run skips the job,
and nothing else.

Skipping it is only safe while three things hold, and each is asserted here:

1. The job is skipped on a dry run and on nothing else, so a tagged release
   still runs it, and even a dry run runs it for an event `ci.yml` does not
   answer (`ready_for_review`), where no gate run would cover it.
2. `release` still needs it, so publication cannot proceed without it.
3. The pull request still runs the same smoke: `ci.yml` calls the Windows gate
   unconditionally, `build-test-windows` carries no condition of its own, and
   its smoke invocation is the release job's, token for token.

Run via ``make test-workflow-contracts``.
"""

import shlex
import typing as typ

from workflow_loading import (
    CI_WINDOWS_WORKFLOW_PATH,
    CI_WORKFLOW_PATH,
    RELEASE_WORKFLOW_PATH,
    REPO_ROOT,
    job_steps,
    load_workflow,
    named_step,
    require_list,
    require_mapping,
    workflow_job,
)

if typ.TYPE_CHECKING:
    from pathlib import Path

SMOKE_JOB = "windows-native-recipe-smoke"
SMOKE_STEP = "Exercise native Windows recipes"
SMOKE_SCRIPT = "./scripts/windows-recipe-smoke.ps1"

#: The only condition the release smoke job may carry. Compared whole, so an
#: appended disjunct or a different output name fails rather than passing.
DRY_RUN_SKIP = (
    "needs.metadata.outputs.dry_run != 'true' "
    "|| github.event.action == 'ready_for_review'"
)

#: The pull-request event types the dry run answers that `ci.yml` does not.
#: The smoke must run for each of them, because no gate run covers it.
UNCOVERED_BY_CI = frozenset({"ready_for_review"})


def smoke_invocation(run: object) -> list[str]:
    r"""Return the smoke script's command line from a PowerShell run block.

    PowerShell continues a line with a trailing backtick, and the gate spells
    the command across three lines that way while the release job folds it
    onto one. Both are joined before splitting, so the two compare as tokens.

    Parameters
    ----------
    run : object
        A step's ``run`` value.

    Returns
    -------
    list[str]
        The tokens of the first line invoking the smoke script, or an empty
        list when there is none.

    Examples
    --------
    >>> smoke_invocation("./scripts/windows-recipe-smoke.ps1 `\n  -Manifest x")
    ['./scripts/windows-recipe-smoke.ps1', '-Manifest', 'x']
    """
    if not isinstance(run, str):
        return []
    joined = run.replace("`\n", " ")
    for line in joined.splitlines():
        if line.strip().startswith(SMOKE_SCRIPT):
            return shlex.split(line)
    return []


def test_the_release_smoke_is_skipped_on_a_dry_run_only() -> None:
    """Skip the smoke on a dry run, and run it on every other release."""
    job = workflow_job(load_workflow(RELEASE_WORKFLOW_PATH), SMOKE_JOB)
    condition = " ".join(str(job.get("if", "")).split())
    assert condition == DRY_RUN_SKIP, (
        f"{SMOKE_JOB} must be skipped exactly when the run is a dry run "
        f"({DRY_RUN_SKIP!r}); a wider condition would skip it on a tagged "
        f"release too, got {job.get('if')!r}"
    )


def _pull_request_types(path: Path) -> frozenset[str]:
    """Return the pull_request activity types one workflow answers."""
    workflow = load_workflow(path)
    triggers = require_mapping(workflow.get("on", workflow.get(True)), "triggers")
    pull_request = require_mapping(triggers.get("pull_request"), "pull_request")
    return frozenset(
        str(kind) for kind in require_list(pull_request.get("types"), "types")
    )


def test_every_skipped_dry_run_event_is_one_ci_also_runs() -> None:
    """Skip the smoke only for pull-request events the gate also answers.

    A dry run on an event `ci.yml` does not answer has no `build-test-windows`
    behind it, so the smoke must run there. The events outside CI's set must
    be exactly the ones the skip condition exempts.
    """
    dry_run = _pull_request_types(REPO_ROOT / ".github/workflows/release-dry-run.yml")
    ci = _pull_request_types(CI_WORKFLOW_PATH)
    assert dry_run - ci == UNCOVERED_BY_CI, (
        f"the dry run answers {sorted(dry_run - ci)} that CI does not; the smoke "
        f"exempts {sorted(UNCOVERED_BY_CI)}"
    )
    for kind in UNCOVERED_BY_CI:
        assert f"github.event.action == '{kind}'" in DRY_RUN_SKIP, (
            f"the smoke must run for {kind}, which no CI run covers"
        )


def test_publication_still_needs_the_smoke() -> None:
    """Keep the smoke a prerequisite of publishing a release."""
    needs = workflow_job(load_workflow(RELEASE_WORKFLOW_PATH), "release").get("needs")
    assert isinstance(needs, list), f"release must list its needs, got {needs!r}"
    assert SMOKE_JOB in needs, (
        f"release must need {SMOKE_JOB}, or a tag could publish without the "
        f"native Windows smoke passing, got {needs!r}"
    )


def test_the_pull_request_runs_the_same_smoke() -> None:
    """Hold the pull-request gate to the smoke the dry run no longer runs.

    Unconditional at both levels, and the same invocation token for token. If
    any of these drifted, a pull request would reach its merge without the
    smoke the dry run used to give it.
    """
    windows_call = workflow_job(load_workflow(CI_WORKFLOW_PATH), "windows")
    condition, called = windows_call.get("if"), windows_call.get("uses")
    # Key absence, not a null value: GitHub reads an empty `if` as false.
    assert "if" not in windows_call, (
        f"ci.yml must call the Windows gate on every run, got {condition!r}"
    )
    assert called == "./.github/workflows/ci-windows.yml", (
        f"ci.yml's windows job must call ci-windows.yml, got {called!r}"
    )
    gate = load_workflow(CI_WINDOWS_WORKFLOW_PATH)
    assert "if" not in workflow_job(gate, "build-test-windows"), (
        "build-test-windows must run whenever the Windows gate does"
    )
    gate_smoke = named_step(job_steps(gate, "build-test-windows"), SMOKE_STEP)
    release = load_workflow(RELEASE_WORKFLOW_PATH)
    release_smoke = named_step(job_steps(release, SMOKE_JOB), SMOKE_STEP)
    gate_command = smoke_invocation(gate_smoke.get("run"))
    assert gate_command, "build-test-windows must invoke the smoke script"
    assert gate_command == smoke_invocation(release_smoke.get("run")), (
        "the pull-request smoke must be the release smoke, token for token: "
        f"gate {gate_command!r}, release "
        f"{smoke_invocation(release_smoke.get('run'))!r}"
    )
    release_shell = release_smoke.get("shell") or _default_shell(release, SMOKE_JOB)
    assert gate_smoke.get("shell") == release_shell == "pwsh", (
        "both smokes must launch Netsuke from PowerShell, got gate "
        f"{gate_smoke.get('shell')!r} and release {release_shell!r}"
    )


def _default_shell(workflow: dict[str, object], job: str) -> object:
    """Return a job's `defaults.run.shell`, or None when it declares none."""
    defaults = workflow_job(workflow, job).get("defaults")
    run = defaults.get("run") if isinstance(defaults, dict) else None
    return run.get("shell") if isinstance(run, dict) else None
