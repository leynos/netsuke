"""Keep release staging read-only and isolate publishing credentials."""

from workflow_loading import (
    REPO_ROOT,
    job_steps,
    load_workflow,
    require_mapping,
    step_index_by_key,
    workflow_job,
)

RELEASE_WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "release.yml"


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
