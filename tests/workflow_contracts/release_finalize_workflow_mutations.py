"""Break one property of the finalization job at a time.

Each mutation changes exactly one thing the job contract asserts - a
permission, a dependency, an artefact selection, the guard, the step order, or
the step list itself - so a test can show the assertion notices the change
rather than passing because nothing varies. The action's mutations live in
``release_finalize_contract_support``; this module is about the job that calls
it.

Run via ``make test-workflow-contracts``.
"""

import pytest
from release_finalize_contract_support import (
    FINALIZE_JOB,
    LOCAL_ACTION,
    TRUSTED_ARTIFACT_VALUE,
    VERIFY_JOB,
)
from workflow_loading import (
    job_steps,
    require_list,
    require_mapping,
    workflow_job,
)

#: The mutations that change who the job depends on, what it may do, or which
#: record it downloads.
_WIRING_MUTATIONS = frozenset({
    "id-token",
    "release-dependency",
    "missing-verifier",
    "artifact-pattern",
    "trusted-literal",
})

#: The mutations that hand the write token to every step beneath a scope.
_TOKEN_SCOPE_MUTATIONS = frozenset({"job-token", "workflow-token"})


def mutate_workflow(workflow: dict[str, object], mutation: str) -> None:
    """Change one dimension of the finalization job's wiring.

    Parameters
    ----------
    workflow
        The workflow document to change in place. The caller deep-copies the
        checked-in workflow first, so a mutation never edits the file.
    mutation
        The mutation id, which selects exactly one change. An id that no branch
        handles fails the test rather than passing silently.
    """
    job = workflow_job(workflow, FINALIZE_JOB)
    if mutation in _WIRING_MUTATIONS:
        _mutate_wiring(workflow, job, mutation)
    elif mutation in _TOKEN_SCOPE_MUTATIONS:
        _mutate_token_scope(workflow, job, mutation)
    elif mutation in {"missing-step-timeout", "verifier-record"}:
        _mutate_finalize_step(workflow, mutation)
    elif mutation in {"step-continue-on-error", "step-disabled"}:
        _mutate_step_controls(workflow, mutation)
    elif mutation in {"guard-bypass", "early-finalize", "extra-edit"}:
        _mutate_bypass(workflow, mutation)
    else:
        pytest.fail(f"unknown workflow mutation {mutation!r}")


def _mutate_wiring(
    workflow: dict[str, object], job: dict[str, object], mutation: str
) -> None:
    """Break one dependency, permission, or artefact-selection property."""
    match mutation:
        case "id-token":
            permissions = require_mapping(job["permissions"], "finalize permissions")
            permissions["id-token"] = "write"
        case "release-dependency":
            needs = require_list(job.get("needs"), "finalize needs")
            job["needs"] = [*needs, "release"]
        case "missing-verifier":
            needs = require_list(job.get("needs"), "finalize needs")
            job["needs"] = [name for name in needs if name != VERIFY_JOB]
        case "artifact-pattern":
            download = _trusted_download(workflow)
            with_values = require_mapping(download["with"], "artifact inputs")
            del with_values["name"]
            with_values["pattern"] = "declared-assets-*"
        case "trusted-literal":
            download = _trusted_download(workflow)
            require_mapping(download["with"], "artifact inputs")["name"] = (
                TRUSTED_ARTIFACT_VALUE
            )
        case other:
            pytest.fail(f"unknown wiring mutation {other!r}")


def _mutate_token_scope(
    workflow: dict[str, object], job: dict[str, object], mutation: str
) -> None:
    """Distribute the write token to every step beneath a scope."""
    match mutation:
        case "job-token":
            # Every step of the job inherits it, including the action's own
            # validate and plan steps, which the action keeps token-free.
            job["env"] = {"GH_TOKEN": "${{ github.token }}"}
        case "workflow-token":
            # Wider still: every job in the workflow inherits it.
            workflow["env"] = {"GH_TOKEN": "${{ github.token }}"}
        case other:
            pytest.fail(f"unknown token-scope mutation {other!r}")


def _mutate_finalize_step(workflow: dict[str, object], mutation: str) -> None:
    """Break one property of the step that invokes the action."""
    local = _finalize_step(workflow)
    match mutation:
        case "missing-step-timeout":
            del local["timeout-minutes"]
        case "verifier-record":
            require_mapping(local["with"], "finalizer inputs")["digest-record-path"] = (
                "verified/verified-digests.json"
            )
        case other:
            pytest.fail(f"unknown finalize-step mutation {other!r}")


def _mutate_step_controls(workflow: dict[str, object], mutation: str) -> None:
    """Let a failed or skipped finalizer leave the job green."""
    local = _finalize_step(workflow)
    match mutation:
        case "step-continue-on-error":
            # The step keeps its signature, its inputs, and its ceiling; only
            # the record of its failure is discarded.
            local["continue-on-error"] = True
        case "step-disabled":
            # The step is still declared, still ordered last, and never runs.
            local["if"] = "false"
        case other:
            pytest.fail(f"unknown step-control mutation {other!r}")


def _mutate_bypass(workflow: dict[str, object], mutation: str) -> None:
    """Let the job run, or the release change, outside the contracted shape."""
    steps = job_steps(workflow, FINALIZE_JOB)
    match mutation:
        case "guard-bypass":
            # Every fragment the assertions could look for survives; only the
            # appended alternative changes what may start the job.
            job = workflow_job(workflow, FINALIZE_JOB)
            job["if"] = f"{job['if']} || true"
        case "early-finalize":
            # The action runs before the records it reconciles arrive: the
            # shape of a step moved up while every name stays right.
            local = _finalize_step(workflow)
            steps.remove(local)
            steps.insert(2, local)
        case "extra-edit":
            # A second, unreviewed step publishes before the action can check
            # the release it is publishing.
            steps.insert(
                2,
                {
                    "name": "Publish the release before reconciling it",
                    "run": 'gh release edit "$RELEASE_TAG" --draft=false',
                },
            )
        case other:
            pytest.fail(f"unknown bypass mutation {other!r}")


def _trusted_download(workflow: dict[str, object]) -> dict[str, object]:
    """Return the job step that downloads the trusted digest record."""
    return next(
        step
        for step in job_steps(workflow, FINALIZE_JOB)
        if str(step.get("uses", "")).startswith("actions/download-artifact@")
        and require_mapping(step.get("with"), "artifact inputs").get("path")
        == "trusted"
    )


def _finalize_step(workflow: dict[str, object]) -> dict[str, object]:
    """Return the job step that invokes the repository's finalization action."""
    return next(
        step
        for step in job_steps(workflow, FINALIZE_JOB)
        if step.get("uses") == LOCAL_ACTION
    )
