"""Load and mutate the package action and job for its release contract.

The contract test needs both documents twice: once to assert their current
shape and once per deliberate mutation, to prove the assertion notices the
change. This module holds the loading, the shared accessors, and the mutation
set, so the test module states the contract while each mutation stays beside
the shape it changes.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
import yaml
from workflow_loading import (
    REPO_ROOT,
    job_steps,
    named_step,
    require_list,
    require_mapping,
    workflow_job,
)

if typ.TYPE_CHECKING:
    from pathlib import Path

PACKAGE_JOB = "package-crate"
PACKAGE_STEP = "Package the crate and record its identity"
CANDIDATE_STEP = "Validate the packaged candidate offline"
ARTIFACT_STEP = "Upload the packaged archive to regather"
ARTIFACT_ACTION = "actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9"
ACTION_PATH: Path = REPO_ROOT / ".github" / "actions" / "package-crate" / "action.yml"
IDENTITY_PATH = "artifacts/package/identity.json"
CRATE_GLOB = "target/package/*.crate"


def load_action() -> dict[str, object]:
    """Read the package action as a checked mapping."""
    return require_mapping(
        yaml.safe_load(ACTION_PATH.read_text(encoding="utf-8")),
        "package-crate action",
    )


def load_action_steps(action: dict[str, object]) -> list[dict[str, object]]:
    """Return the action's steps with each step narrowed to a mapping."""
    runs = require_mapping(action.get("runs"), "action runs")
    raw_steps = require_list(runs.get("steps"), "action steps")
    return [require_mapping(step, "action step") for step in raw_steps]


def package_action_step(action: dict[str, object]) -> dict[str, object]:
    """Return the uniquely named package action step."""
    return named_step(load_action_steps(action), "Package crate")


def candidate_action_step(action: dict[str, object]) -> dict[str, object]:
    """Return the uniquely named offline candidate action step."""
    return named_step(load_action_steps(action), CANDIDATE_STEP)


def artifact_inputs(workflow: dict[str, object]) -> dict[str, object]:
    """Return the crate package artefact's ``with`` mapping."""
    return require_mapping(
        named_step(job_steps(workflow, PACKAGE_JOB), ARTIFACT_STEP).get("with"),
        "artifact inputs",
    )


def mutate_action(action: dict[str, object], mutation: str) -> None:
    """Apply one deliberate mutation to a package-action contract dimension."""
    match mutation:
        case "input":
            inputs = require_mapping(action["inputs"], "inputs")
            require_mapping(inputs["version"], "version input")["required"] = False
        case "environment":
            package = package_action_step(action)
            require_mapping(package["env"], "package environment")[
                "RELEASE_VERSION"
            ] = "wrong"
        case "command":
            package = package_action_step(action)
            run = str(package["run"])
            package["run"] = run.replace("--target-dir target", "--target-dir build")
        case "output":
            outputs = require_mapping(action["outputs"], "outputs")
            require_mapping(outputs["crate-name"], "crate-name output")["value"] = (
                "${{ steps.other.outputs.crate-name }}"
            )
        case "candidate":
            candidate = candidate_action_step(action)
            run = str(candidate["run"])
            candidate["run"] = run.replace(
                "release_mode=(--rehearsal)", 'release_mode=(--tag "$RELEASE_TAG")'
            )
        case "candidate-continue-on-error":
            candidate_action_step(action)["continue-on-error"] = True
        case "candidate-gate":
            candidate_action_step(action)["if"] = "always()"
        case other:
            pytest.fail(f"unknown action mutation {other!r}")


#: Mutations that change whether a package-job step admits a failure.
_STEP_GATE_MUTATIONS = frozenset({
    "package-continue-on-error",
    "artifact-gate",
    "artifact-continue-on-error",
})

#: Mutations that change the artefact a downstream job receives.
_ARTIFACT_MUTATIONS = frozenset({
    "artifact-action",
    "artifact-name",
    "artifact-path",
    "artifact-missing-policy",
})


def _mutate_step_gate(workflow: dict[str, object], mutation: str) -> None:
    """Change one package-job step's success gate or failure policy."""
    steps = job_steps(workflow, PACKAGE_JOB)
    match mutation:
        case "package-continue-on-error":
            named_step(steps, PACKAGE_STEP)["continue-on-error"] = True
        case "artifact-gate":
            named_step(steps, ARTIFACT_STEP)["if"] = "always()"
        case "artifact-continue-on-error":
            named_step(steps, ARTIFACT_STEP)["continue-on-error"] = True
        case other:
            pytest.fail(f"unknown step-gate mutation {other!r}")


def _mutate_artifact(workflow: dict[str, object], mutation: str) -> None:
    """Change one dimension of the uploaded crate artefact."""
    match mutation:
        case "artifact-action":
            named_step(job_steps(workflow, PACKAGE_JOB), ARTIFACT_STEP)["uses"] = (
                "actions/upload-artifact@main"
            )
        case "artifact-name":
            artifact_inputs(workflow)["name"] = "wrong-name"
        case "artifact-path":
            artifact_inputs(workflow)["path"] = IDENTITY_PATH
        case "artifact-missing-policy":
            artifact_inputs(workflow)["if-no-files-found"] = "warn"
        case other:
            pytest.fail(f"unknown artefact mutation {other!r}")


def mutate_workflow(workflow: dict[str, object], mutation: str) -> None:
    """Apply one deliberate mutation to package-job wiring."""
    if mutation in _STEP_GATE_MUTATIONS:
        _mutate_step_gate(workflow, mutation)
        return
    if mutation in _ARTIFACT_MUTATIONS:
        _mutate_artifact(workflow, mutation)
        return
    match mutation:
        case "dependency":
            workflow_job(workflow, PACKAGE_JOB)["needs"] = []
        case "permission":
            workflow_job(workflow, PACKAGE_JOB)["permissions"] = {"contents": "write"}
        case other:
            pytest.fail(f"unknown workflow mutation {other!r}")
