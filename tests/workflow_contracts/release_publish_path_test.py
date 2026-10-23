"""Prove dry runs reach release staging without publishing."""

from release_publish_path_contract import (
    CALLER_WORKFLOW_PATH,
    check_release_publish_path,
)
from workflow_loading import (
    PACKAGE_WORKFLOW_PATH,
    RELEASE_WORKFLOW_PATH,
    load_workflow,
)


def test_release_dry_run_reaches_the_staging_path() -> None:
    """Run the release-path contract over every dry-run and publish scenario."""
    violations = check_release_publish_path(
        load_workflow(RELEASE_WORKFLOW_PATH),
        load_workflow(CALLER_WORKFLOW_PATH),
        load_workflow(PACKAGE_WORKFLOW_PATH),
    )
    assert not violations, "release publish-path contract violations:\n" + "\n".join(
        violations
    )
