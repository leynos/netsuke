"""Prove the release-path contract rejects each named regression."""

import pytest
from action_run_sources import action_sources
from release_publish_crates_io_contracts import ACTION_PATH
from release_publish_path_contract import (
    CALLER_WORKFLOW_PATH,
    check_release_publish_path,
)
from release_publish_path_mutations import (
    MUTATION_LABELS,
    MutationError,
    MutationIssue,
    WorkflowInputs,
    mutate_workflows,
)
from workflow_loading import (
    PACKAGE_WORKFLOW_PATH,
    RELEASE_WORKFLOW_PATH,
    load_workflow,
)


def test_mutation_error_preserves_standard_exception_arguments() -> None:
    """Keep the issue and detail available through AssertionError.args."""
    detail = ("guard expression", 0)
    error = MutationError(MutationIssue.PATTERN_MISMATCH, detail)

    assert error.args == (MutationIssue.PATTERN_MISMATCH, detail), (
        "AssertionError args should retain mutation context"
    )


@pytest.fixture(scope="module")
def workflows() -> WorkflowInputs:
    """Parse release workflows and the publisher action once."""
    return WorkflowInputs(
        release=load_workflow(RELEASE_WORKFLOW_PATH),
        caller=load_workflow(CALLER_WORKFLOW_PATH),
        build=load_workflow(PACKAGE_WORKFLOW_PATH),
        action=action_sources()[ACTION_PATH],
    )


@pytest.mark.parametrize("label", MUTATION_LABELS, ids=MUTATION_LABELS)
def test_release_path_contract_rejects_each_regression(
    label: str, workflows: WorkflowInputs
) -> None:
    """Require each mutation's affected scenario or invariant to fail by name."""
    case = mutate_workflows(label, workflows)
    violations = check_release_publish_path(
        case.release, case.caller, case.build, case.action
    )
    for scenario, contract in case.expected_violations:
        prefix = f"{contract}:" if scenario == "static" else f"{scenario}.{contract}:"
        assert any(violation.startswith(prefix) for violation in violations), (
            f"{case.label} should report {prefix!r}; got {violations!r}"
        )
