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
import re

import pytest
from release_finalize_action_contract import (
    FINALIZER_SCRIPT,
    assert_action_contract,
    assert_adapter_pins_its_closure,
)
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
    load_action,
    mutate_action,
)
from release_finalize_job_expectations import (
    EXPECTED_JOB_GUARD,
    EXPECTED_JOB_STEPS,
    step_signature,
)
from release_finalize_workflow_mutations import mutate_workflow
from release_publish_needs import job_names
from workflow_loading import (
    RELEASE_WORKFLOW_PATH,
    REPO_ROOT,
    job_steps,
    load_workflow,
    require_mapping,
    step_index_by_key,
    workflow_job,
)

#: The names a write token is conventionally stored under. A scope that
#: declares one of these hands it to every step beneath it.
_TOKEN_NAMES = frozenset({"GH_TOKEN", "GITHUB_TOKEN"})

#: The expressions that resolve to a write token, whatever name they are
#: stored under, so renaming the variable does not evade the check.
_TOKEN_EXPRESSION = re.compile(r"\bgithub\.token\b|\bsecrets\b")


def _workflow() -> dict[str, object]:
    """Read the release workflow for the finalization job contract."""
    return load_workflow(RELEASE_WORKFLOW_PATH)


def _assert_no_inherited_token(
    workflow: dict[str, object], job: dict[str, object]
) -> None:
    """Require the token to reach only the steps that ask for it.

    A workflow- or job-level ``env`` is inherited by every step, including the
    action's validate and plan steps, which the action contract deliberately
    keeps token-free. The action receives the credential as an input, so no
    enclosing scope has a reason to define one.
    """
    for scope, label in ((workflow, "workflow"), (job, "job")):
        environment = require_mapping(scope.get("env") or {}, f"{label} environment")
        leaked = sorted(
            name
            for name, value in environment.items()
            if name.upper() in _TOKEN_NAMES
            or _TOKEN_EXPRESSION.search(str(value)) is not None
        )
        assert not leaked, (
            f"the {label} must not define a token variable: every step inherits "
            f"it, including the ones the action keeps token-free, got {leaked}"
        )


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
    # The whole expression, not its fragments: appending `|| true` keeps every
    # fragment intact while letting a failed verifier start the job.
    guard = " ".join(str(job.get("if")).split())
    assert guard == EXPECTED_JOB_GUARD, (
        "the job's guard must be exactly the contracted expression; "
        f"`!cancelled()` suppresses the implicit success check, got {guard!r}"
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
    """Check the job's guard, permissions, step sequence, and local action."""
    job = workflow_job(workflow, FINALIZE_JOB)
    _assert_job_guard(job)
    _assert_no_inherited_token(workflow, job)
    steps = job_steps(workflow, FINALIZE_JOB)
    signatures = [step_signature(step) for step in steps]
    assert signatures == list(EXPECTED_JOB_STEPS), (
        "the job may run only its checkout, its interpreter, the two record "
        "downloads, and the action, in that order; a reordering runs the "
        "action without its records and an extra step could change the "
        f"release before the action checks it, got {signatures!r}"
    )
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
    for values in downloads:
        assert set(values) == {"name", "path"}, (
            "a record download may select only by exact name, and may choose "
            "no other attempt's artefact: a pattern or a re-run override would "
            f"let the cross-check agree with the wrong record, got {values!r}"
        )

    local = steps[step_index_by_key(steps, "uses", LOCAL_ACTION)]
    _assert_local_action(local)


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
    # The step's signature, its inputs, and its ceiling all survive either
    # control, so neither is visible to the assertions above: one discards the
    # record of a refusal, and the other skips the publication outright.
    assert "continue-on-error" not in step, (
        "the step that invokes the action must fail the job when the action "
        "refuses: continue-on-error would report an unpublished release as a "
        "passing step"
    )
    assert "if" not in step, (
        "the step that invokes the action must not be conditional: a false "
        "condition skips the finalization the job exists to perform"
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
        "commented-plan-reset",
        "source-build-allowed",
        "gh-in-run",
        "extra-command",
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
        "job-token",
        "workflow-token",
        "missing-step-timeout",
        "verifier-record",
        "step-continue-on-error",
        "step-disabled",
        "guard-bypass",
        "early-finalize",
        "extra-edit",
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


def test_the_dependency_cutoff_contract_rejects_a_removed_cutoff() -> None:
    """Prove the closure assertion notices a block whose cutoff was removed."""
    source = (REPO_ROOT / FINALIZER_SCRIPT).read_text(encoding="utf-8")
    assert_adapter_pins_its_closure(source)
    stripped = "\n".join(
        line for line in source.splitlines() if not line.startswith("# exclude-newer")
    )
    with pytest.raises(AssertionError):
        assert_adapter_pins_its_closure(stripped)


def test_the_dependency_cutoff_contract_rejects_a_relocated_cutoff() -> None:
    """Prove the closure assertion reads the block, not the whole file."""
    source = (REPO_ROOT / FINALIZER_SCRIPT).read_text(encoding="utf-8")
    lines = source.splitlines()
    cutoff = [line for line in lines if line.startswith("# exclude-newer")]
    assert len(cutoff) == 1, "the control assumes exactly one cutoff line"
    remainder = [line for line in lines if line != cutoff[0]]
    closing = remainder.index("# ///")
    relocated = [*remainder[: closing + 1], cutoff[0], *remainder[closing + 1 :]]

    with pytest.raises(AssertionError):
        assert_adapter_pins_its_closure("\n".join(relocated))
