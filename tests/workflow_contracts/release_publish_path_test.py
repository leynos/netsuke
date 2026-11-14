"""Prove dry runs reach release staging without publishing."""

import pytest
from release_publish_path_contract import (
    CALLER_WORKFLOW_PATH,
    check_release_publish_path,
)
from release_publish_path_scenarios import _needs_results
from workflow_loading import (
    PACKAGE_WORKFLOW_PATH,
    RELEASE_WORKFLOW_PATH,
    load_workflow,
)


def test_a_scenario_cannot_name_an_unmodelled_dependency() -> None:
    """Refuse an outcome for a job the scenarios do not model.

    Without the check the override lands beside the dependency the scenario
    meant to change, leaving every real dependency at ``success``. The scenario
    then passes while asserting the opposite of its name, which is the failure
    mode the whole table exists to avoid.
    """
    with pytest.raises(ValueError, match="unknown dependenc"):
        _needs_results(outcomes={"build-linx": "failure"})


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
