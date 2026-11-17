"""Prove dry runs reach release staging without publishing."""

import copy

import pytest
from release_publish_path_authorization import (
    AUTHORIZATION_SCRIPT,
    AUTHORIZATION_STEP,
    check_authorization_wiring,
)
from release_publish_path_contract import (
    CALLER_WORKFLOW_PATH,
    check_release_publish_path,
)
from release_publish_path_scenario_model import needs_results
from workflow_loading import (
    PACKAGE_WORKFLOW_PATH,
    RELEASE_WORKFLOW_PATH,
    job_steps,
    load_workflow,
    require_mapping,
    step_index_by_key,
)


def _metadata_step(release: dict[str, object], step_id: str) -> dict[str, object]:
    """Return one step of the release workflow's metadata job."""
    steps = job_steps(release, "metadata")
    return steps[step_index_by_key(steps, "id", step_id)]


#: Run bodies that name the policy without executing it. ``comment-only`` hides
#: it behind ``#``; ``argument-only`` passes it to ``python -c`` as an argument.
_UNEXECUTED_RUNS = (
    pytest.param(
        f"uv run --no-project --python 3.14 python -c 'pass'  # {AUTHORIZATION_SCRIPT}",
        id="comment-only",
    ),
    pytest.param(
        f"uv run --no-project --python 3.14 python -c 'pass' {AUTHORIZATION_SCRIPT}",
        id="argument-only",
    ),
)


@pytest.mark.parametrize("run", _UNEXECUTED_RUNS)
def test_the_authorization_step_must_execute_the_policy(run: str) -> None:
    """Refuse a run body that names the policy without running it.

    A membership test accepts both a trailing comment and a script handed to
    ``python -c`` as an argument, so the step would run something else while
    the policy's name stayed in the workflow.
    """
    release = copy.deepcopy(load_workflow(RELEASE_WORKFLOW_PATH))
    step = _metadata_step(release, AUTHORIZATION_STEP)
    step["run"] = run
    violations: list[str] = []

    check_authorization_wiring(release, violations)

    assert any(v.startswith("authorization.run") for v in violations), (
        "a mention that never executes the policy must not satisfy the "
        f"authorization run check: {violations}"
    )


def test_the_authorization_step_id_must_match_exactly() -> None:
    """Refuse an id that merely contains the expected one.

    GitHub binds a step's outputs to its own id, so a step renamed to
    ``release_authorization_unused`` still looks wired to a substring lookup
    while no consumer reads its outputs.
    """
    release = copy.deepcopy(load_workflow(RELEASE_WORKFLOW_PATH))
    step = _metadata_step(release, AUTHORIZATION_STEP)
    step["id"] = f"{AUTHORIZATION_STEP}_unused"
    violations: list[str] = []

    check_authorization_wiring(release, violations)

    assert any(v.startswith("authorization.step") for v in violations), (
        f"a renamed step id must not satisfy the lookup: {violations}"
    )


def test_the_authorization_step_must_run_unconditionally() -> None:
    """Refuse a decision step that a condition can skip.

    The gates are checked against synthetic outputs, so a step guarded by
    ``${{ false }}`` would pass every other check while never writing them.
    """
    release = copy.deepcopy(load_workflow(RELEASE_WORKFLOW_PATH))
    step = _metadata_step(release, AUTHORIZATION_STEP)
    step["if"] = "${{ false }}"
    violations: list[str] = []

    check_authorization_wiring(release, violations)

    assert any(v.startswith("authorization.condition") for v in violations), (
        f"a conditional decision step must be reported: {violations}"
    )


def test_the_version_gate_treats_a_string_true_as_enabled() -> None:
    """Refuse version checking switched on by the string Actions would pass.

    GitHub Actions materialises ``with:`` values as strings, so a ``check-tag``
    of ``"true"`` enables the same check the boolean does. A gate that rejects
    only ``True`` would leave a denied run checking the version tag.
    """
    release = copy.deepcopy(load_workflow(RELEASE_WORKFLOW_PATH))
    step = _metadata_step(release, "ensure_version")
    with_values = require_mapping(step.get("with"), "ensure_version.with")
    with_values["check-tag"] = "${{ 'true' }}"
    violations: list[str] = []

    check_authorization_wiring(release, violations)

    assert any(v.startswith("authorization.version-tag-gate") for v in violations), (
        f"the string 'true' must be read as enabled version checking: {violations}"
    )


def test_a_scenario_cannot_name_an_unmodelled_dependency() -> None:
    """Refuse an outcome for a job the scenarios do not model.

    Without the check the override lands beside the dependency the scenario
    meant to change, leaving every real dependency at ``success``. The scenario
    then passes while asserting the opposite of its name, which is the failure
    mode the whole table exists to avoid.
    """
    with pytest.raises(ValueError, match="unknown dependenc"):
        needs_results(outcomes={"build-linx": "failure"})


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
