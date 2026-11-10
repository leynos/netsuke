"""Hold the release-finalization job to its contract.

Finalization is the one release stage whose effect cannot be undone, so the
properties this suite pins are the ones that keep that effect safe: it runs
only after every producer and the verifier have succeeded, it reconciles
against the trusted pre-upload digest record rather than the verifier's output,
it holds no credential beyond the release token, and it makes exactly one
call that can change anything. Each property is asserted against the checked
documents and then against a deliberate mutation, so the assertion is shown to
notice the change rather than merely to pass today.

The action's own steps are asserted in ``release_finalize_action_contract``;
this module holds the job that calls it, and the mutation cases that prove
both halves fire.

Run via ``make test-workflow-contracts``.
"""

import copy

import pytest
from release_finalize_action_contract import assert_action_contract
from release_finalize_contract_support import (
    ARTIFACT_ACTION,
    CHECKOUT_ACTION,
    EXPECTED_JOB_INPUTS,
    EXPECTED_NEEDS,
    EXPECTED_PERMISSIONS,
    FINALIZE_JOB,
    FINALIZE_STEP_TIMEOUT_MINUTES,
    JOB_TIMEOUT_MINUTES,
    LOCAL_ACTION,
    SETUP_UV_ACTION,
    TRUSTED_ARTIFACT_OUTPUT,
    load_action,
    mutate_action,
    mutate_workflow,
)
from release_publish_needs import job_names
from workflow_loading import (
    RELEASE_WORKFLOW_PATH,
    job_steps,
    load_workflow,
    require_mapping,
    step_index_by_key,
    workflow_job,
)


def _workflow() -> dict[str, object]:
    """Read the release workflow for the finalization job contract."""
    return load_workflow(RELEASE_WORKFLOW_PATH)


def _assert_job_guard(job: dict[str, object]) -> None:
    """Require the dependency list, success guard, and privilege of the job."""
    needs = job_names(job.get("needs"))
    assert needs is not None, (
        "the job's needs must be a job name or a list of job names, got "
        f"{job.get('needs')!r}"
    )
    assert sorted(needs) == sorted(EXPECTED_NEEDS), (
        f"the job must depend on exactly {EXPECTED_NEEDS}, got {needs!r}"
    )
    assert len(needs) == len(EXPECTED_NEEDS), (
        "a repeated dependency is an error GitHub rejects, not an extra witness"
    )
    guard = str(job.get("if"))
    assert "!cancelled()" in guard, "the job must keep its explicit cancellation guard"
    assert "needs.metadata.outputs.should_publish == 'true'" in guard, (
        "a rehearsal must never finalize a release"
    )
    for name in EXPECTED_NEEDS:
        if name == "metadata":
            continue
        assert f"needs.{name}.result == 'success'" in guard, (
            f"the guard must name {name}'s success; `!cancelled()` suppresses "
            "the implicit success check"
        )
    permissions = require_mapping(job.get("permissions"), "finalize permissions")
    assert permissions == EXPECTED_PERMISSIONS, (
        "finalization needs the release token and nothing else"
    )
    timeout = job.get("timeout-minutes")
    assert isinstance(timeout, int), "the job must carry a timeout-minutes"
    assert timeout == JOB_TIMEOUT_MINUTES, (
        f"the job's ceiling must stay {JOB_TIMEOUT_MINUTES} minutes, got {timeout}"
    )


def _assert_job_contract(workflow: dict[str, object]) -> None:
    """Check the job's guard, permissions, artefact trust, and local action."""
    _assert_job_guard(workflow_job(workflow, FINALIZE_JOB))
    steps = job_steps(workflow, FINALIZE_JOB)
    checkout = steps[step_index_by_key(steps, "uses", CHECKOUT_ACTION)]
    checkout_with = require_mapping(checkout.get("with"), "checkout inputs")
    assert checkout_with.get("persist-credentials") is False, (
        "the candidate checkout must not persist credentials"
    )
    assert checkout_with.get("ref") == (
        "${{ needs.metadata.outputs.candidate_commit }}"
    ), "the job must finalize with the candidate's own scripts"

    setup_uv = steps[step_index_by_key(steps, "uses", SETUP_UV_ACTION)]
    uv_with = require_mapping(setup_uv.get("with"), "setup-uv inputs")
    assert uv_with.get("python-version") == "3.14", (
        "the release helpers run on the pinned baseline interpreter"
    )

    downloads = [
        require_mapping(step.get("with"), "artifact inputs")
        for step in steps
        if str(step.get("uses", "")).startswith(ARTIFACT_ACTION)
    ]
    trusted = [values for values in downloads if values.get("path") == "trusted"]
    assert len(trusted) == 1, "exactly one download must feed the trusted record"
    assert trusted[0].get("name") == TRUSTED_ARTIFACT_OUTPUT, (
        "the digest record must be the artefact the publisher recorded, not one "
        "recomputed from this job's attempt"
    )
    assert "pattern" not in trusted[0], (
        "a pattern could select another run's digest record"
    )
    package = [values for values in downloads if values.get("path") != "trusted"]
    assert len(package) == 1, "exactly one download must feed the package record"
    assert package[0].get("name") == "release-crate-package", (
        "the package record must come from the packaging stage's artefact"
    )
    assert "pattern" not in package[0], (
        "a pattern could select another run's package record"
    )

    local = steps[step_index_by_key(steps, "uses", LOCAL_ACTION)]
    _assert_local_action(local)
    assert steps.index(local) > max(steps.index(checkout), steps.index(setup_uv)), (
        "the finalization action must run after its checkout and tools"
    )


def _assert_local_action(step: dict[str, object]) -> None:
    """Require the action's inputs and the aggregate ceiling around them."""
    inputs = require_mapping(step.get("with"), "finalizer inputs")
    assert inputs == EXPECTED_JOB_INPUTS, (
        "the job must forward the contracted finalization inputs"
    )
    assert inputs["digest-record-path"] == "trusted/trusted.json", (
        "reconciliation must read the trusted pre-upload record; the verifier's "
        "output agrees with GitHub by construction"
    )
    timeout = step.get("timeout-minutes")
    assert isinstance(timeout, int), (
        "the step that invokes the action must carry the aggregate ceiling, "
        "because the action's own steps cannot"
    )
    assert timeout == FINALIZE_STEP_TIMEOUT_MINUTES, (
        f"the action's ceiling must stay {FINALIZE_STEP_TIMEOUT_MINUTES} minutes, "
        f"got {timeout!r}"
    )
    assert timeout < JOB_TIMEOUT_MINUTES, (
        "the action's ceiling must leave the job's own timeout room to report it"
    )


def _assert_complete_contract(
    action: dict[str, object], workflow: dict[str, object]
) -> None:
    """Require the action and the job that calls it to agree."""
    assert_action_contract(action)
    _assert_job_contract(workflow)


@pytest.mark.parametrize(
    "mutation",
    [
        "token-on-validate",
        "token-on-plan",
        "unbounded-command",
        "aggregate-bound-exceeded",
        "stale-plan-kept",
        "missing-plan-guard",
        "gh-in-run",
        "wrong-output",
    ],
)
def test_action_mutations_are_rejected(mutation: str) -> None:
    """Prove the action contract detects each finalizer mutation."""
    action = copy.deepcopy(load_action())
    workflow = copy.deepcopy(_workflow())
    mutate_action(action, mutation)

    with pytest.raises(AssertionError):
        _assert_complete_contract(action, workflow)


@pytest.mark.parametrize(
    "mutation",
    [
        "id-token",
        "release-dependency",
        "missing-verifier",
        "artifact-pattern",
        "trusted-literal",
        "missing-step-timeout",
        "verifier-record",
    ],
)
def test_workflow_mutations_are_rejected(mutation: str) -> None:
    """Prove the job contract detects each dependency and trust mutation."""
    action = copy.deepcopy(load_action())
    workflow = copy.deepcopy(_workflow())
    mutate_workflow(workflow, mutation)

    with pytest.raises(AssertionError):
        _assert_complete_contract(action, workflow)


def test_the_action_and_job_satisfy_the_complete_contract() -> None:
    """Require the checked-in action and workflow to agree on the contract."""
    _assert_complete_contract(load_action(), _workflow())
