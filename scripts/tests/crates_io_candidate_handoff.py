"""Model the package job's runner so a test can observe the crate hand-off.

The workflow contract reads YAML and the action test runs one step body;
neither shows whether a rejected candidate stops the artefact the next job
consumes. This module runs the package job's steps in order under GitHub's
failure semantics: a step runs only when its gate admits it, a failing step
fails the job unless the step declares ``continue-on-error``, and the upload
action is replaced by a local copy of the files it names. Packaging itself is
not executed; the caller supplies the workspace the packaging step would have
written, and ``test_package_crate_cli`` covers the packager.

Run via ``make test-release-publication``.
"""

import dataclasses as dc
import pathlib
import shutil
import subprocess  # ruff: ignore[suspicious-subprocess-import] - a step runs as a child.
import typing as typ

import pytest
import yaml

REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "release.yml"
ACTION_MANIFEST = REPO_ROOT / ".github" / "actions" / "package-crate" / "action.yml"

PACKAGE_JOB = "package-crate"
PACKAGE_JOB_STEP = "Package the crate and record its identity"
CANDIDATE_STEP = "Validate the packaged candidate offline"
LOCAL_ACTION = "./.github/actions/package-crate"
UPLOAD_STEP = "Upload the packaged archive to regather"
UPLOAD_ACTION = "actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9"

#: The script the packaging step runs. The runner substitutes it rather than
#: executing Cargo, so this text identifies the step it must skip.
_PACKAGER = "package_crate_cli.py"


@dc.dataclass(frozen=True, slots=True)
class StepOutcome:
    """Report one modelled step's success and the names it handed off."""

    succeeded: bool
    uploaded: tuple[str, ...] = ()


@dc.dataclass(frozen=True, slots=True)
class JobOutcome:
    """Report the package job's failure state and handed-off artefact."""

    failed: bool
    uploaded: tuple[str, ...]


@dc.dataclass(frozen=True, slots=True)
class Runner:
    """Hold the action steps and upload destination one run should use."""

    action_steps: tuple[dict[str, object], ...]
    upload_root: pathlib.Path


@dc.dataclass(frozen=True, slots=True)
class JobContext:
    """Hold the workspace and step environment one run should use."""

    workspace: pathlib.Path
    environment: dict[str, str]


def _mapping(raw: object, description: str) -> dict[str, object]:
    """Narrow an untyped YAML node to a mapping."""
    assert isinstance(raw, dict), f"the {description} must be a mapping"
    return typ.cast("dict[str, object]", raw)


def _steps(raw: object, description: str) -> list[dict[str, object]]:
    """Narrow an untyped YAML step list to a list of mappings."""
    assert isinstance(raw, list), f"the {description} must be a step list"
    return [_mapping(step, "step") for step in raw]


def load_workflow() -> dict[str, object]:
    """Return the release workflow as a checked mapping."""
    loaded = yaml.safe_load(WORKFLOW_PATH.read_text(encoding="utf-8"))
    return _mapping(loaded, "release workflow")


def load_action_steps() -> list[dict[str, object]]:
    """Return the package action's declared steps."""
    runs = _mapping(_mapping(load_action(), "package action").get("runs"), "runs")
    return _steps(runs.get("steps"), "action runs")


def load_action() -> dict[str, object]:
    """Return the package action manifest as a checked mapping."""
    loaded = yaml.safe_load(ACTION_MANIFEST.read_text(encoding="utf-8"))
    return _mapping(loaded, "package action")


def hand_off_steps(workflow: dict[str, object]) -> list[dict[str, object]]:
    """Return the job steps from packaging through the crate upload.

    The harness owns the hand-off boundary. Steps before packaging only
    prepare the runner, and steps after the upload record timing and
    observability for an outcome the job has already decided.

    Returns
    -------
    list[dict[str, object]]
        The job steps from packaging through the crate upload, in order.
    """
    jobs = _mapping(workflow.get("jobs"), "workflow jobs")
    steps = _steps(_mapping(jobs.get(PACKAGE_JOB), "package job").get("steps"), "steps")
    names = [step.get("name") for step in steps]
    for required in (PACKAGE_JOB_STEP, UPLOAD_STEP):
        assert required in names, f"the package job must declare {required!r}"
    return steps[names.index(PACKAGE_JOB_STEP) : names.index(UPLOAD_STEP) + 1]


def named_step(steps: list[dict[str, object]], name: str) -> dict[str, object]:
    """Return the uniquely named step."""
    matching = [step for step in steps if step.get("name") == name]
    assert len(matching) == 1, f"expected exactly one step named {name!r}"
    return matching[0]


def _is_continue_on_error(step: dict[str, object]) -> bool:
    """Whether a step lets the job continue after it fails."""
    value = step.get("continue-on-error", False)
    assert isinstance(value, bool), (
        "a computed continue-on-error is outside what this runner models"
    )
    return value


def _step_runs(step: dict[str, object], *, failed: bool) -> bool:
    """Apply GitHub's step gate, whose default is ``success()``."""
    condition = step.get("if")
    if condition is None:
        return not failed
    text = str(condition).strip()
    if text == "always()":
        return True
    if text == "success()":
        return not failed
    return pytest.fail(f"the runner cannot model the step condition {text!r}")


def _run_shell(
    step: dict[str, object], workspace: pathlib.Path, environment: dict[str, str]
) -> StepOutcome:
    """Run a shell step body as the runner would."""
    run = step.get("run")
    assert isinstance(run, str), "a step without an action must carry a script"
    bash = shutil.which("bash")
    assert bash is not None, "the package job's steps run under Bash"
    completed = subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - fixed script; shell is disabled.
        [bash, "--noprofile", "--norc", "-c", run],
        capture_output=True,
        check=False,
        cwd=workspace,
        env=environment,
        text=True,
    )
    return StepOutcome(succeeded=completed.returncode == 0)


def _run_upload(
    step: dict[str, object], workspace: pathlib.Path, upload_root: pathlib.Path
) -> StepOutcome:
    """Copy the upload step's paths locally instead of to the service."""
    inputs = _mapping(step.get("with"), "upload inputs")
    name = inputs.get("name")
    assert isinstance(name, str), "the upload step must name its artefact"
    paths = inputs.get("path")
    assert isinstance(paths, str), "the upload step must list the paths it sends"
    destination = upload_root / name
    copied: list[str] = []
    for pattern in paths.splitlines():
        for path in sorted(workspace.glob(pattern.strip())):
            destination.mkdir(parents=True, exist_ok=True)
            _ = shutil.copyfile(path, destination / path.name)
            copied.append(path.name)
    if not copied and inputs.get("if-no-files-found") == "error":
        return StepOutcome(succeeded=False)
    return StepOutcome(succeeded=True, uploaded=tuple(copied))


def _run_package_action(
    steps: list[dict[str, object]], context: JobContext, runner: Runner
) -> StepOutcome:
    """Run the composite action, substituting the packager it wraps."""
    packaging = steps[0]
    run = packaging.get("run")
    assert isinstance(run, str), "the crate packager must carry a script"
    assert _PACKAGER in run, "the substituted first step must be the crate packager"
    outcome = _run_steps(steps[1:], context, runner)
    return StepOutcome(succeeded=not outcome.failed, uploaded=outcome.uploaded)


def _run_step(
    step: dict[str, object], context: JobContext, runner: Runner
) -> StepOutcome:
    """Model one step: a shell body, the package action, or the upload."""
    uses = step.get("uses")
    if uses == LOCAL_ACTION:
        return _run_package_action(list(runner.action_steps), context, runner)
    if uses == UPLOAD_ACTION:
        return _run_upload(step, context.workspace, runner.upload_root)
    if uses is not None:
        # Checkout and toolchain setup have no bearing on the hand-off.
        return StepOutcome(succeeded=True)
    return _run_shell(step, context.workspace, context.environment)


def _run_steps(
    steps: list[dict[str, object]], context: JobContext, runner: Runner
) -> JobOutcome:
    """Run steps in order under GitHub's failure semantics."""
    failed = False
    uploaded: list[str] = []
    for step in steps:
        if not _step_runs(step, failed=failed):
            continue
        outcome = _run_step(step, context, runner)
        uploaded.extend(outcome.uploaded)
        if not outcome.succeeded and not _is_continue_on_error(step):
            failed = True
    return JobOutcome(failed=failed, uploaded=tuple(uploaded))


def run_package_job(
    context: JobContext, runner: Runner, workflow: dict[str, object]
) -> JobOutcome:
    """Run the package job's hand-off steps as the runner would."""
    return _run_steps(hand_off_steps(workflow), context, runner)


def mutate_hand_off(
    workflow: dict[str, object],
    action_steps: list[dict[str, object]],
    mutation: str,
) -> None:
    """Apply one deliberate failure-bypassing edit to the hand-off.

    The runner must notice each of these, which is what proves it observes the
    gating rather than assuming it.
    """
    match mutation:
        case "upload-always":
            named_step(hand_off_steps(workflow), UPLOAD_STEP)["if"] = "always()"
        case "package-continue-on-error":
            named_step(hand_off_steps(workflow), PACKAGE_JOB_STEP)[
                "continue-on-error"
            ] = True
        case "candidate-continue-on-error":
            named_step(action_steps, CANDIDATE_STEP)["continue-on-error"] = True
        case other:
            pytest.fail(f"unknown hand-off mutation {other!r}")
