"""Keep release staging read-only and isolate publishing credentials."""

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

    for job_name in ("release", "publish-release"):
        steps = job_steps(release, job_name)
        checkout_index = step_index_by_key(steps, "uses", "actions/checkout@")
        checkout_with = require_mapping(
            steps[checkout_index].get("with"), f"{job_name} checkout.with"
        )
        assert checkout_with.get("persist-credentials") is False, (
            f"{job_name} scripts must not inherit checkout credentials"
        )
