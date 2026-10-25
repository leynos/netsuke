"""Hold the crate packaging action and job to their release contract.

The crate archive is handed to later release jobs through a GitHub artefact.
The contract pins the action's interface, command, identity forwarding, job
dependency, permission, and artefact contents so a workflow edit cannot leave
the next job with an unbound archive or identity record.

Run via ``make test-workflow-contracts``.
"""

import copy
import shlex
import typing as typ

import pytest
import yaml
from workflow_loading import (
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

PACKAGE_JOB = "package-crate"
PACKAGE_STEP = "Package the crate and record its identity"
ARTIFACT_STEP = "Upload the packaged archive to regather"
ARTIFACT_ACTION = "actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9"
ACTION_PATH: Path = REPO_ROOT / ".github" / "actions" / "package-crate" / "action.yml"
IDENTITY_PATH = "artifacts/package/identity.json"
CRATE_GLOB = "target/package/*.crate"

EXPECTED_INPUTS = {
    "tag": False,
    "dry-run": False,
    "commit": True,
    "package-name": True,
    "version": True,
    "bin-name": True,
}
EXPECTED_ENVIRONMENT = {
    "RELEASE_TAG": "${{ inputs.tag }}",
    "RELEASE_DRY_RUN": "${{ inputs['dry-run'] }}",
    "RELEASE_COMMIT": "${{ inputs.commit }}",
    "RELEASE_PACKAGE_NAME": "${{ inputs['package-name'] }}",
    "RELEASE_VERSION": "${{ inputs.version }}",
    "RELEASE_BIN_NAME": "${{ inputs['bin-name'] }}",
}
EXPECTED_OUTPUTS = {
    "identity-path": "${{ steps.package.outputs.identity-path }}",
    "crate-path": "${{ steps.package.outputs.crate-path }}",
    "crate-name": "${{ steps.package.outputs.crate-name }}",
}
EXPECTED_JOB_INPUTS = {
    "tag": "${{ needs.metadata.outputs.release_tag }}",
    "dry-run": "${{ needs.metadata.outputs.should_publish != 'true' }}",
    "commit": "${{ github.sha }}",
    "package-name": "${{ needs.metadata.outputs.package_name }}",
    "version": "${{ needs.metadata.outputs.version }}",
    "bin-name": "${{ needs.metadata.outputs.bin_name }}",
}


def _action() -> dict[str, object]:
    """Read the package action as a checked mapping."""
    return require_mapping(
        yaml.safe_load(ACTION_PATH.read_text(encoding="utf-8")),
        "package-crate action",
    )


def _action_steps(action: dict[str, object]) -> list[dict[str, object]]:
    """Return the action's steps with each step narrowed to a mapping."""
    runs = require_mapping(action.get("runs"), "action runs")
    raw_steps = require_list(runs.get("steps"), "action steps")
    return [require_mapping(step, "action step") for step in raw_steps]


def _package_action_step(action: dict[str, object]) -> dict[str, object]:
    """Return the uniquely named package action step."""
    return named_step(_action_steps(action), "Package crate")


def _run_tokens(run: str) -> list[str]:
    """Return shell tokens from the complete CLI invocation."""
    lines = run.splitlines()
    start = next(index for index, line in enumerate(lines) if "uv run " in line)
    command = lines[start].strip()
    while command.endswith("\\"):
        start += 1
        command = f"{command[:-1]} {lines[start].strip()}"
    return shlex.split(command)


def _workflow() -> dict[str, object]:
    """Read the release workflow for the package job contract."""
    return load_workflow(RELEASE_WORKFLOW_PATH)


def _assert_action_contract(action: dict[str, object]) -> None:
    """Check the action's inputs, environment, command, and outputs."""
    inputs = require_mapping(action.get("inputs"), "action inputs")
    assert set(inputs) == set(EXPECTED_INPUTS), (
        "the package action must expose exactly the contracted inputs"
    )
    for name, required in EXPECTED_INPUTS.items():
        spec = require_mapping(inputs[name], f"{name} input")
        assert spec.get("required") is required, f"{name} required flag changed"
    tag_default = require_mapping(inputs["tag"], "tag input").get("default")
    assert isinstance(tag_default, str), "the tag default must be a string"
    assert not tag_default, "the tag input must have no default"
    assert (
        require_mapping(inputs["dry-run"], "dry-run input").get("default") == "false"
    ), "the dry-run input must default to false"

    outputs = require_mapping(action.get("outputs"), "action outputs")
    assert set(outputs) == set(EXPECTED_OUTPUTS), (
        "the package action must expose exactly the contracted outputs"
    )
    for name, value in EXPECTED_OUTPUTS.items():
        output = require_mapping(outputs[name], f"{name} output")
        assert output.get("value") == value, f"{name} output mapping changed"

    steps = _action_steps(action)
    package = named_step(steps, "Package crate")
    assert package.get("id") == "package", "the package step id must remain stable"
    assert package.get("shell") == "bash", "the action step must run under Bash"
    environment = require_mapping(package.get("env"), "package environment")
    assert environment == EXPECTED_ENVIRONMENT, (
        "the action must forward inputs through the contracted environment names"
    )
    run = package.get("run")
    assert isinstance(run, str), "the package command must be a shell script"
    assert run.splitlines()[0] == "set -euo pipefail", (
        "the package command must enable strict shell handling"
    )
    assert _run_tokens(run) == [
        "uv",
        "run",
        "--no-project",
        "--python",
        "3.14",
        "scripts/package_crate_cli.py",
        "${release_selector[@]}",
        "--commit",
        "$RELEASE_COMMIT",
        "--package-name",
        "$RELEASE_PACKAGE_NAME",
        "--version",
        "$RELEASE_VERSION",
        "--bin-name",
        "$RELEASE_BIN_NAME",
        "--manifest",
        "Cargo.toml",
        "--target-dir",
        "target",
        "--output",
        IDENTITY_PATH,
    ], "the action must invoke the package CLI with every identity input"


def _assert_workflow_contract(workflow: dict[str, object]) -> None:
    """Check package-job dependencies, privilege, and hand-off artefact."""
    job = workflow_job(workflow, PACKAGE_JOB)
    assert job.get("needs") == "metadata", (
        "the package job must depend on validated release metadata"
    )
    assert job.get("permissions") == {"contents": "read"}, (
        "the package job must have read-only repository permissions"
    )
    steps = job_steps(workflow, PACKAGE_JOB)
    package = named_step(steps, PACKAGE_STEP)
    assert package.get("uses") == "./.github/actions/package-crate", (
        "the package job must use the repository's package action"
    )
    assert (
        require_mapping(package.get("with"), "package inputs") == EXPECTED_JOB_INPUTS
    ), "the package job must forward the validated metadata inputs"

    artifact = named_step(steps, ARTIFACT_STEP)
    assert artifact.get("uses") == ARTIFACT_ACTION, (
        "the package archive must use the pinned artifact upload action"
    )
    artifact_inputs = require_mapping(artifact.get("with"), "artifact inputs")
    assert artifact_inputs.get("name") == "release-crate-package", (
        "the package artifact name must remain stable for downstream jobs"
    )
    artifact_path = artifact_inputs.get("path")
    assert isinstance(artifact_path, str), "artifact paths must be serialized as text"
    assert artifact_path.splitlines() == [IDENTITY_PATH, CRATE_GLOB], (
        "the package artifact must contain the identity record and crate archive"
    )
    assert artifact_inputs.get("if-no-files-found") == "error", (
        "a missing package artifact must fail the workflow"
    )
    assert steps.index(package) < steps.index(artifact), (
        "the package action must produce the files before upload"
    )


def _mutate_action(action: dict[str, object], mutation: str) -> None:
    """Apply one deliberate mutation to a package-action contract dimension."""
    match mutation:
        case "input":
            inputs = require_mapping(action["inputs"], "inputs")
            require_mapping(inputs["version"], "version input")["required"] = False
        case "environment":
            package = _package_action_step(action)
            require_mapping(package["env"], "package environment")[
                "RELEASE_VERSION"
            ] = "wrong"
        case "command":
            package = _package_action_step(action)
            run = str(package["run"])
            package["run"] = run.replace("--target-dir target", "--target-dir build")
        case "output":
            outputs = require_mapping(action["outputs"], "outputs")
            require_mapping(outputs["crate-name"], "crate-name output")["value"] = (
                "${{ steps.other.outputs.crate-name }}"
            )
        case other:
            pytest.fail(f"unknown action mutation {other!r}")


def _mutate_workflow(workflow: dict[str, object], mutation: str) -> None:
    """Apply one deliberate mutation to package-job wiring."""
    match mutation:
        case "dependency":
            workflow_job(workflow, PACKAGE_JOB)["needs"] = []
        case "permission":
            workflow_job(workflow, PACKAGE_JOB)["permissions"] = {"contents": "write"}
        case "artifact-action":
            named_step(job_steps(workflow, PACKAGE_JOB), ARTIFACT_STEP)["uses"] = (
                "actions/upload-artifact@main"
            )
        case "artifact-name":
            _artifact_inputs(workflow)["name"] = "wrong-name"
        case "artifact-path":
            _artifact_inputs(workflow)["path"] = IDENTITY_PATH
        case "artifact-missing-policy":
            _artifact_inputs(workflow)["if-no-files-found"] = "warn"
        case other:
            pytest.fail(f"unknown workflow mutation {other!r}")


def _assert_complete_package_contract(
    action: dict[str, object], workflow: dict[str, object]
) -> None:
    """Require both the action and workflow to satisfy the package contract."""
    _assert_action_contract(action)
    _assert_workflow_contract(workflow)


@pytest.mark.parametrize("mutation", ["input", "environment", "command", "output"])
def test_action_mutations_are_rejected(mutation: str) -> None:
    """Prove the action contract detects mutations to its four dimensions."""
    action = copy.deepcopy(_action())
    workflow = copy.deepcopy(_workflow())
    _mutate_action(action, mutation)

    with pytest.raises(AssertionError):
        _assert_complete_package_contract(action, workflow)


@pytest.mark.parametrize(
    "mutation",
    [
        "dependency",
        "permission",
        "artifact-action",
        "artifact-name",
        "artifact-path",
        "artifact-missing-policy",
    ],
)
def test_workflow_mutations_are_rejected(mutation: str) -> None:
    """Prove the workflow contract detects each package-job mutation."""
    action = copy.deepcopy(_action())
    workflow = copy.deepcopy(_workflow())
    _mutate_workflow(workflow, mutation)

    with pytest.raises(AssertionError):
        _assert_complete_package_contract(action, workflow)


def _artifact_inputs(workflow: dict[str, object]) -> dict[str, object]:
    """Return the crate package artefact's ``with`` mapping."""
    return require_mapping(
        named_step(job_steps(workflow, PACKAGE_JOB), ARTIFACT_STEP).get("with"),
        "artifact inputs",
    )


def test_the_action_and_job_satisfy_the_complete_package_contract() -> None:
    """Require the checked-in action and workflow to agree on the full contract."""
    _assert_complete_package_contract(_action(), _workflow())
