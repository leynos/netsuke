"""Prove the release-path contract rejects each named regression."""

import pytest
from release_publish_path_mutations import MUTATION_LABELS, mutate_workflows
from release_publish_path_test import CALLER_WORKFLOW_PATH, check_release_publish_path
from workflow_loading import (
    PACKAGE_WORKFLOW_PATH,
    RELEASE_WORKFLOW_PATH,
    load_workflow,
)


@pytest.fixture(scope="module")
def workflows() -> tuple[dict[str, object], dict[str, object], dict[str, object]]:
    """Parse the release, caller, and reusable build workflows once."""
    return (
        load_workflow(RELEASE_WORKFLOW_PATH),
        load_workflow(CALLER_WORKFLOW_PATH),
        load_workflow(PACKAGE_WORKFLOW_PATH),
    )


@pytest.mark.parametrize("label", MUTATION_LABELS, ids=MUTATION_LABELS)
def test_release_path_contract_rejects_each_regression(
    label: str,
    workflows: tuple[dict[str, object], dict[str, object], dict[str, object]],
) -> None:
    """Require each mutation's affected scenario and step to fail by name."""
    case = mutate_workflows(label, *workflows)
    violations = check_release_publish_path(case.release, case.caller, case.build)
    for scenario, contract in case.expected_violations:
        prefix = f"{scenario}.{contract}:"
        assert any(violation.startswith(prefix) for violation in violations), (
            f"{case.label} should report {prefix!r}; got {violations!r}"
        )
