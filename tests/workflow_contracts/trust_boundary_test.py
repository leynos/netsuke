"""Hold the PR coverage secret boundary and poisoning-regression harness.

The untrusted CI run can write ``BASH_ENV`` or ``GITHUB_PATH`` into its own
per-job environment files. This structural harness proves it exports only the
LCOV artefact, while the trusted ``workflow_run`` starts another job on a fresh
runner, consumes no persisted shell state, and exposes the secret only to its
submission action after validation has completed.
"""

import typing as typ

from trust_boundary_invariants import (
    CREDENTIAL_ENVIRONMENT_KEY,
    REQUIRED_SECRET_JOB_PERMISSIONS,
    TOKEN_PRESENCE_GUARD,
    TRUSTED_CHECKOUT_REF,
    contains_text,
    is_isolated_secret_job,
    job_references_secret,
)
from workflow_loading import (
    COVERAGE_PR_WORKFLOW_PATH,
    REPO_ROOT,
    job_steps,
    load_workflow,
    require_mapping,
    workflow_job,
)

if typ.TYPE_CHECKING:
    from pathlib import Path

UNTRUSTED_CI_PATH = REPO_ROOT / ".github" / "workflows" / "ci.yml"
ARTEFACT_NAME = "pr-coverage-lcov"
ARTEFACT_PATH = "lcov.info"
SUBMISSION_STEP = "Check coverage against CodeScene gates"
EXPECTED_SUBMISSION_CONDITION = (
    "github.event.workflow_run.conclusion == 'success' && "
    "github.event.workflow_run.event == 'pull_request' && "
    "github.event.workflow_run.head_repository.full_name == github.repository"
)
EXPECTED_PR_ARTEFACT_STEP = {
    "name": "Upload PR coverage artefact",
    "uses": "actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a",
    "with": {
        "name": ARTEFACT_NAME,
        "path": ARTEFACT_PATH,
        "retention-days": 3,
        "if-no-files-found": "error",
    },
}


def _workflow_paths() -> list[Path]:
    """Return every checked-in GitHub Actions workflow path."""
    return sorted((REPO_ROOT / ".github" / "workflows").glob("*.yml"))


def _is_pull_request_workflow(workflow: dict[str, object]) -> bool:
    """Return whether one parsed workflow has a pull-request trigger."""
    triggers = require_mapping(workflow.get("on"), "workflow trigger")
    return "pull_request" in triggers


def test_pull_request_workflows_never_reference_codescene_secret() -> None:
    """Forbid the secret in parsed values and raw PR-workflow text alike."""
    for workflow_path in _workflow_paths():
        workflow = load_workflow(workflow_path)
        if not _is_pull_request_workflow(workflow):
            continue
        assert not contains_text(workflow, CREDENTIAL_ENVIRONMENT_KEY), (
            f"{workflow_path.name} must not reference "
            f"{CREDENTIAL_ENVIRONMENT_KEY} in parsed values"
        )
        assert CREDENTIAL_ENVIRONMENT_KEY not in workflow_path.read_text(
            encoding="utf-8"
        ), (
            f"{workflow_path.name} must not reference "
            f"{CREDENTIAL_ENVIRONMENT_KEY} in raw YAML"
        )


def test_submission_workflow_uses_trusted_workflow_run_boundary() -> None:
    """Require completed CI PR runs to cross into a distinct trusted workflow."""
    workflow = load_workflow(COVERAGE_PR_WORKFLOW_PATH)
    triggers = require_mapping(workflow.get("on"), "coverage submission trigger")
    workflow_run = require_mapping(triggers.get("workflow_run"), "workflow_run trigger")
    assert workflow_run == {"workflows": ["CI"], "types": ["completed"]}, (
        "submission must run only after completed CI"
    )

    job = workflow_job(workflow, "submit-coverage")
    condition = str(job.get("if", ""))
    assert condition == EXPECTED_SUBMISSION_CONDITION, (
        "submission eligibility must require successful same-repository PR CI"
    )
    assert job.get("permissions") == REQUIRED_SECRET_JOB_PERMISSIONS, (
        "submission must retain least-privilege permissions"
    )


def test_secret_job_checks_out_only_trusted_tooling_and_validates_first() -> None:
    """Keep untrusted code and artefact bytes out of secret-bearing execution."""
    workflow = load_workflow(COVERAGE_PR_WORKFLOW_PATH)
    job = workflow_job(workflow, "submit-coverage")
    steps = job_steps(workflow, "submit-coverage")
    assert is_isolated_secret_job(job, steps), (
        "the CodeScene job must isolate its step-local credential"
    )

    checkout_steps = [
        step for step in steps if "actions/checkout@" in str(step.get("uses", ""))
    ]
    assert len(checkout_steps) == 1, "the trusted job must have one checkout"
    checkout_with = require_mapping(checkout_steps[0].get("with"), "trusted checkout")
    assert checkout_with.get("ref") == TRUSTED_CHECKOUT_REF, (
        "the trusted job must check out the default branch only"
    )

    names = [step.get("name") for step in steps]
    validation_index = names.index("Validate hostile coverage artefact")
    submission_index = names.index(SUBMISSION_STEP)
    assert validation_index < submission_index, (
        "hostile artefact validation must precede the secret-bearing action"
    )

    submission = steps[submission_index]
    assert submission.get("if") == TOKEN_PRESENCE_GUARD, (
        "the submission must skip when the credential is absent"
    )
    assert submission.get("env") == {
        CREDENTIAL_ENVIRONMENT_KEY: "${{ secrets.CS_ACCESS_TOKEN }}"
    }, "the credential must be supplied only through the step environment"
    assert not job.get("env"), "the CodeScene secret must not be job scoped"


def test_poisoned_untrusted_environment_cannot_cross_to_submission_runner() -> None:
    """Keep BASH_ENV and GITHUB_PATH poisoning inside the untrusted runner."""
    ci_workflow = load_workflow(UNTRUSTED_CI_PATH)
    ci_steps = job_steps(ci_workflow, "build-test")
    assert not job_references_secret(ci_steps, CREDENTIAL_ENVIRONMENT_KEY), (
        "untrusted PR CI must not receive the CodeScene credential"
    )

    artefact_steps = [
        step for step in ci_steps if step.get("name") == "Upload PR coverage artefact"
    ]
    assert artefact_steps == [EXPECTED_PR_ARTEFACT_STEP], (
        "untrusted CI may export only the bounded LCOV artefact"
    )

    trusted_text = COVERAGE_PR_WORKFLOW_PATH.read_text(encoding="utf-8")
    for poisoned_name in ("BASH_ENV", "GITHUB_PATH"):
        assert poisoned_name not in trusted_text, (
            f"trusted workflow must not consume untrusted {poisoned_name} state"
        )
    assert "workflow_run" in trusted_text, "submission must start in a fresh workflow"


def test_isolated_secret_job_detects_non_env_secret_references() -> None:
    """Reject secret references placed outside a step's environment mapping."""
    isolated: dict[str, object] = {"permissions": REQUIRED_SECRET_JOB_PERMISSIONS}
    guarded_secret_step: dict[str, object] = {
        "name": "Submit",
        "if": TOKEN_PRESENCE_GUARD,
        "env": {CREDENTIAL_ENVIRONMENT_KEY: "${{ secrets.CS_ACCESS_TOKEN }}"},
    }

    assert is_isolated_secret_job(isolated, [guarded_secret_step]), (
        "the guarded step-local carrier must satisfy the secret-job boundary"
    )

    mutations: list[dict[str, object]] = [
        {
            "name": "Submit",
            "if": TOKEN_PRESENCE_GUARD,
            "run": "echo ${{ secrets.CS_ACCESS_TOKEN }}",
        },
        {
            "name": "Submit",
            "if": TOKEN_PRESENCE_GUARD,
            "with": {"access-token": "${{ secrets.CS_ACCESS_TOKEN }}"},
        },
    ]
    for mutation in mutations:
        assert not is_isolated_secret_job(isolated, [mutation]), (
            f"secret reference in {sorted(mutation)} must break isolation"
        )
