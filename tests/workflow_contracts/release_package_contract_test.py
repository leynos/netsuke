"""Hold the crate packaging action and job to their release contract.

The crate archive is handed to later release jobs through a GitHub artefact.
The contract pins the action's interface, command, identity forwarding, job
dependency, permission, and artefact contents so a workflow edit cannot leave
the next job with an unbound archive or identity record.

Run via ``make test-workflow-contracts``.
"""

import copy
import re
import shlex

import pytest
from release_package_contract_support import (
    ARTIFACT_ACTION,
    ARTIFACT_STEP,
    CANDIDATE_STEP,
    CRATE_GLOB,
    IDENTITY_PATH,
    PACKAGE_JOB,
    PACKAGE_STEP,
    load_action,
    load_action_steps,
    mutate_action,
    mutate_workflow,
)
from workflow_loading import (
    RELEASE_WORKFLOW_PATH,
    job_steps,
    load_workflow,
    named_step,
    require_mapping,
    workflow_job,
)

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
# The candidate check validates identity and bytes, so it needs every package
# input except the binary name, which only the packaging step records.
EXPECTED_CANDIDATE_ENVIRONMENT = {
    name: value
    for name, value in EXPECTED_ENVIRONMENT.items()
    if name != "RELEASE_BIN_NAME"
}
EXPECTED_OUTPUTS = {
    "identity-path": "${{ steps.package.outputs.identity-path }}",
    "crate-path": "${{ steps.package.outputs.crate-path }}",
    "crate-name": "${{ steps.package.outputs.crate-name }}",
    # The job reads these rather than the action's own outcome, because the
    # action bundles packaging with candidate validation: without them a
    # rejected candidate would be recorded as a packaging failure.
    "crate-packaging-outcome": "${{ steps.package.outcome }}",
    "crate-packaging-started-ns": "${{ steps.package.outputs.started_ns }}",
    "crate-packaging-ended-ns": "${{ steps.package.outputs.ended_ns }}",
    "crate-candidate-validation-outcome": "${{ steps.candidate.outcome }}",
    "crate-candidate-validation-started-ns": (
        "${{ steps.candidate.outputs.started_ns }}"
    ),
    "crate-candidate-validation-ended-ns": "${{ steps.candidate.outputs.ended_ns }}",
}
EXPECTED_JOB_INPUTS = {
    "tag": "${{ needs.metadata.outputs.release_tag }}",
    "dry-run": "${{ needs.metadata.outputs.should_publish != 'true' }}",
    "commit": "${{ needs.metadata.outputs.candidate_commit }}",
    "package-name": "${{ needs.metadata.outputs.package_name }}",
    "version": "${{ needs.metadata.outputs.version }}",
    "bin-name": "${{ needs.metadata.outputs.bin_name }}",
}


def _assert_failure_propagates(step: dict[str, object], description: str) -> None:
    """Require a step to keep the default success gate and to fail its job.

    The publisher gates on ``needs.package-crate.result == 'success'``, so a
    step that tolerates failure, or that runs without looking at the previous
    step's outcome, would hand a rejected candidate to the next job while the
    workflow still reported a completed package job.
    """
    assert "continue-on-error" not in step, (
        f"{description} must fail the job rather than report success anyway"
    )
    assert "if" not in step, (
        f"{description} must keep the default success gate on the step before it"
    )


def _run_tokens(run: str) -> list[str]:
    """Return shell tokens from the complete CLI invocation."""
    lines = run.splitlines()
    start = next(index for index, line in enumerate(lines) if "uv run " in line)
    command = lines[start].strip()
    while command.endswith("\\"):
        start += 1
        command = f"{command[:-1]} {lines[start].strip()}"
    return shlex.split(command)


_TIMING_MARKERS = (
    'printf \'started_ns=%s\\n\' "$(date +%s%N)" >>"$GITHUB_OUTPUT"',
    'printf \'ended_ns=%s\\n\' "$(date +%s%N)" >>"$GITHUB_OUTPUT"',
    "trap record_ended_ns EXIT",
)


def _assert_step_reports_timing(step: dict[str, object], description: str) -> None:
    """Require a step to report a start time and an end time from an exit trap.

    An end time written by the script's last statement disappears exactly when
    the script fails, which is the case the phase's duration exists to explain.
    The trap also disarms itself before writing, so an error inside the handler
    cannot recurse.
    """
    run = step.get("run")
    assert isinstance(run, str), f"the {description} command must be a shell script"
    for marker in _TIMING_MARKERS:
        assert marker in run, f"the {description} must report its own timing"
    assert "trap - EXIT" in run, f"the {description} trap must disarm itself"


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

    steps = load_action_steps(action)
    package = named_step(steps, "Package crate")
    assert package.get("id") == "package", "the package step id must remain stable"
    assert package.get("shell") == "bash", "the action step must run under Bash"
    _assert_failure_propagates(package, "the packaging step")
    _assert_step_reports_timing(package, "packaging step")
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

    _assert_candidate_step(steps, package)


def _assert_candidate_step(
    steps: list[dict[str, object]], package: dict[str, object]
) -> None:
    """Check the offline candidate step runs after packaging, without network."""
    # The publisher repeats this validation after the artefact hand-off, so this
    # step proves the local bytes and record agree before upload. It must stay
    # offline: a token or registry request here would let a pull-request lane
    # reach the exchange it is meant to avoid.
    candidate = named_step(steps, CANDIDATE_STEP)
    assert candidate.get("id") == "candidate", "the candidate step id must be stable"
    assert candidate.get("shell") == "bash", "the candidate step must run under Bash"
    assert steps.index(package) < steps.index(candidate), (
        "the candidate must be validated after it is packaged"
    )
    _assert_failure_propagates(candidate, "the candidate step")
    _assert_step_reports_timing(candidate, "candidate step")
    environment = require_mapping(candidate.get("env"), "candidate environment")
    assert environment == EXPECTED_CANDIDATE_ENVIRONMENT, (
        "the candidate step must forward the identity inputs it validates"
    )
    run = candidate.get("run")
    assert isinstance(run, str), "the candidate command must be a shell script"
    assert run.splitlines()[0] == "set -euo pipefail", (
        "the candidate command must enable strict shell handling"
    )
    assert _run_tokens(run) == [
        "uv",
        "run",
        "--no-project",
        "--python",
        "3.14",
        "scripts/crates_io_candidate_cli.py",
        "check",
        "--identity",
        IDENTITY_PATH,
        "--archive-directory",
        "target/package",
        "${release_mode[@]}",
        "--commit",
        "$RELEASE_COMMIT",
        "--package-name",
        "$RELEASE_PACKAGE_NAME",
        "--version",
        "$RELEASE_VERSION",
        "--output",
        "$RUNNER_TEMP/crates-io-candidate.json",
    ], "the candidate step must validate the packaged archive and identity"
    # Each pattern names one case arm and the assignment on that same line. A
    # search for the assignments alone would accept a swapped mapping.
    assert re.search(r"true\)\s+(release_mode=\(--rehearsal\))", run, re.MULTILINE), (
        "a dry run must validate a tagless rehearsal candidate"
    )
    assert re.search(
        r'false\)\s+(release_mode=\(--tag "\$RELEASE_TAG"\))', run, re.MULTILINE
    ), "a publishing run must validate the candidate against its release tag"
    # A denylist over the whole step, not only the tokenized command: a later
    # line could otherwise add its own fetch. It catches the named routes, so
    # it supports the offline claim without being a proof of it.
    for forbidden in ("--token", "TOKEN", "curl", "wget", "http", "crates.io"):
        assert forbidden not in run, (
            f"the offline candidate step must not reach the registry: {forbidden}"
        )


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
    _assert_failure_propagates(package, "the package job's action step")
    _assert_failure_propagates(artifact, "the package upload step")


def _assert_complete_package_contract(
    action: dict[str, object], workflow: dict[str, object]
) -> None:
    """Require both the action and workflow to satisfy the package contract."""
    _assert_action_contract(action)
    _assert_workflow_contract(workflow)


@pytest.mark.parametrize(
    "mutation",
    [
        "input",
        "environment",
        "command",
        "output",
        "timing",
        "candidate-output",
        "candidate",
        "candidate-continue-on-error",
        "candidate-gate",
    ],
)
def test_action_mutations_are_rejected(mutation: str) -> None:
    """Prove the action contract detects mutations to each of its dimensions."""
    action = copy.deepcopy(load_action())
    workflow = copy.deepcopy(_workflow())
    mutate_action(action, mutation)

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
        "package-continue-on-error",
        "artifact-gate",
        "artifact-continue-on-error",
    ],
)
def test_workflow_mutations_are_rejected(mutation: str) -> None:
    """Prove the workflow contract detects each package-job mutation."""
    action = copy.deepcopy(load_action())
    workflow = copy.deepcopy(_workflow())
    mutate_workflow(workflow, mutation)

    with pytest.raises(AssertionError):
        _assert_complete_package_contract(action, workflow)


def test_the_action_and_job_satisfy_the_complete_package_contract() -> None:
    """Require the checked-in action and workflow to agree on the full contract."""
    _assert_complete_package_contract(load_action(), _workflow())
