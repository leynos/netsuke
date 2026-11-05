"""Keep release staging read-only and isolate publishing credentials."""

from copy import deepcopy

from action_run_sources import action_sources, action_steps
from release_publish_crates_io_contracts import (
    ACTION_PATH,
    check_crates_io_action_contract,
)
from workflow_loading import (
    RELEASE_WORKFLOW_PATH,
    job_steps,
    load_workflow,
    require_mapping,
    step_index_by_key,
    workflow_job,
)


def test_release_workflow_denies_default_token_permissions() -> None:
    """Jobs must opt in to token scopes instead of inheriting GitHub defaults."""
    release = load_workflow(RELEASE_WORKFLOW_PATH)
    assert release.get("permissions") == {}, (
        "release.yml must deny default token scopes at workflow level"
    )
    jobs = require_mapping(release.get("jobs"), "release workflow jobs")
    for job_name, raw_job in jobs.items():
        job = require_mapping(raw_job, f"release job {job_name}")
        assert "permissions" in job, (
            f"{job_name} must declare the token scopes it requires"
        )


def test_staging_is_read_only_and_only_publication_can_write() -> None:
    """Keep the rehearsal job read-only and scope writes to publication."""
    release = load_workflow(RELEASE_WORKFLOW_PATH)
    _assert_github_release_scopes(release)
    _assert_crates_io_contract(release)
    _assert_checkouts_do_not_persist_credentials(release)


def test_verified_digest_output_comes_from_the_verification_step() -> None:
    """Reject an action output that reports the candidate digest as verified."""
    release = load_workflow(RELEASE_WORKFLOW_PATH)
    action = deepcopy(action_sources()[ACTION_PATH])
    outputs = require_mapping(action.get("outputs"), "publisher action.outputs")
    verified = require_mapping(
        outputs.get("verified-sha256"), "publisher verified-sha256 output"
    )
    verified["value"] = "${{ steps.candidate.outputs.sha256 }}"
    violations: list[str] = []

    check_crates_io_action_contract(release, action, violations)

    assert any(
        "action-output.verified-sha256" in violation for violation in violations
    ), "the published digest must come from the downloaded-byte verification step"


def test_rejected_classification_exports_outputs_before_failure() -> None:
    """Guard telemetry outputs when registry classification rejects mutation."""
    release = load_workflow(RELEASE_WORKFLOW_PATH)
    action = deepcopy(action_sources()[ACTION_PATH])
    decision = next(
        step
        for step in action_steps(ACTION_PATH, action)
        if step.get("id") == "decision"
    )
    decision["run"] = str(decision.get("run", "")).replace(
        'printf \'state=%s\\n\' "$state" >>"$GITHUB_OUTPUT"\n', ""
    )
    violations: list[str] = []

    check_crates_io_action_contract(release, action, violations)

    assert any(
        "classify: require fail-closed decision outputs" in item for item in violations
    ), "the classifier contract must require rejected-state telemetry"


def _assert_github_release_scopes(release: dict[str, object]) -> None:
    """Keep GitHub release write access in its dedicated publishing job."""
    staging = workflow_job(release, "release")
    staging_permissions = require_mapping(
        staging.get("permissions"), "release staging permissions"
    )
    assert staging_permissions.get("contents") == "read", (
        "the staging rehearsal must not receive contents write access"
    )

    publication = workflow_job(release, "publish-release")
    publication_permissions = require_mapping(
        publication.get("permissions"), "release publication permissions"
    )
    assert publication_permissions.get("contents") == "write", (
        "the publication job must receive contents write access"
    )


def _assert_crates_io_contract(release: dict[str, object]) -> None:
    """Use the shared contract to enforce OIDC scope and secret isolation."""
    action = action_sources()[ACTION_PATH]
    violations: list[str] = []
    check_crates_io_action_contract(release, action, violations)
    permission_violations = [
        violation
        for violation in violations
        if any(
            field in violation
            for field in (".permissions:", ".environment:", ".token:", ".secrets:")
        )
    ]
    assert permission_violations == [], (
        "the shared contract must enforce publisher permissions and credentials"
    )


def _assert_checkouts_do_not_persist_credentials(release: dict[str, object]) -> None:
    """Disable checkout credential persistence for staging and both publishers."""
    for job_name in ("release", "publish-release", "publish-crates-io"):
        steps = job_steps(release, job_name)
        checkout_index = step_index_by_key(steps, "uses", "actions/checkout@")
        checkout_with = require_mapping(
            steps[checkout_index].get("with"), f"{job_name} checkout.with"
        )
        assert checkout_with.get("persist-credentials") is False, (
            f"{job_name} scripts must not inherit checkout credentials"
        )
