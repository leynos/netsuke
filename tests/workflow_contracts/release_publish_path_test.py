"""Prove dry runs reach release staging without publishing."""

import pytest
from release_admission_gate_contract import (
    CHECKOUT_ACTION,
    GATE_SCRIPT,
    UV_ISOLATION_FLAGS,
    verifier_gate_command,
)
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


def test_a_relative_verifier_path_names_the_same_gate_command() -> None:
    """Read ``./gate`` as the same verifier directory as ``gate``.

    One rule validates the checkout's ``path`` and another rebuilds the gate
    command from the same declaration. Comparing the raw spelling in one place
    and the normalised spelling in the other made a correct workflow that
    spelled the path ``./gate`` fail both, so the expectation is derived
    through the same normalisation the validator applies.
    """
    steps = [{"uses": f"{CHECKOUT_ACTION}@v4", "with": {"path": "./gate"}}]
    assert verifier_gate_command(steps) == (
        f"uv run {UV_ISOLATION_FLAGS} --script gate/{GATE_SCRIPT}"
    ), "a leading './' in the checkout path must not change the gate command"


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
