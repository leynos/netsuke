"""Test the release publish-path contract and the authorization wiring.

The cases cover two promises: a dry run still reaches release staging without
writing a release, and the authorization step closes every write gate for a run
its policy denies. ``release_publish_path_authorization`` owns the wiring
checks, ``release_publish_path_contract`` composes them with the publication
paths, and both are exercised here against the real workflow.

A table-driven contract can pass while the table itself is wrong, so these
cases pin what a green run would otherwise leave implicit: the wiring checks
must refuse a step that names the authorization script without running it, an
id that only contains the expected one, or a decision step a condition can
skip. The dependency outcomes the scenarios declare come from
``needs_results`` in ``release_publish_path_scenario_model``, which refuses an
outcome for a job the scenarios do not model, and the rows themselves live in
``release_publish_path_scenarios``. The workflow documents are parsed from the
repository by ``workflow_loading``, so the contract is held to the real
release, caller and package workflows rather than to a copy of them.

Run via ``make test-workflow-contracts``.
"""

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
#: it behind ``#``; ``argument-only`` passes it to ``python -c`` as an argument;
#: ``split-command`` wraps the expected words across two shell commands, which
#: the runner executes as two separate lines rather than one invocation;
#: ``suffixed-filename`` names a different file that a comment-stripping
#: tokeniser would shorten to the expected path.
_UNEXECUTED_RUNS = (
    pytest.param(
        f"uv run --no-project --python 3.14 python -c 'pass'  # {AUTHORIZATION_SCRIPT}",
        id="comment-only",
    ),
    pytest.param(
        f"uv run --no-project --python 3.14 python -c 'pass' {AUTHORIZATION_SCRIPT}",
        id="argument-only",
    ),
    pytest.param(
        f"uv run\n--no-project --python 3.14 {AUTHORIZATION_SCRIPT}",
        id="split-command",
    ),
    pytest.param(
        f"uv run --no-project --python 3.14 {AUTHORIZATION_SCRIPT}#disabled",
        id="suffixed-filename",
    ),
)


@pytest.mark.parametrize("run", _UNEXECUTED_RUNS)
def test_the_authorization_step_must_execute_the_policy(run: str) -> None:
    """Refuse a run body that names the policy without running it.

    A membership test accepts a trailing comment, a script handed to
    ``python -c`` as an argument, words split across two shell commands, and a
    suffixed filename a comment-stripping tokeniser shortens to the expected
    path, so the step would run something else while the policy's name stayed
    in the workflow.
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


def test_the_step_order_check_refuses_a_decoy_id() -> None:
    """Refuse ordering that a merely similar id in the right place satisfies.

    A substring lookup finds ``release_authorization_unused`` where the real
    step used to sit and reports the order as correct, even though the step that
    decides writes now runs after the gates it is meant to close.
    """
    release = copy.deepcopy(load_workflow(RELEASE_WORKFLOW_PATH))
    steps = job_steps(release, "metadata")
    index = step_index_by_key(steps, "id", AUTHORIZATION_STEP)
    real = steps.pop(index)
    decoy = copy.deepcopy(real)
    decoy["id"] = f"{AUTHORIZATION_STEP}_unused"
    steps.insert(index, decoy)
    steps.append(real)
    violations: list[str] = []

    check_authorization_wiring(release, violations)

    assert any(v.startswith("authorization.order") for v in violations), (
        f"a decoy id before the gates must not satisfy the ordering check: {violations}"
    )


def test_the_authorization_step_id_must_be_unique() -> None:
    """Refuse two steps sharing the decision's id.

    GitHub cannot bind an output to an id declared twice, and a lookup that took
    the first match could read a step the runner rejects outright.
    """
    release = copy.deepcopy(load_workflow(RELEASE_WORKFLOW_PATH))
    steps = job_steps(release, "metadata")
    steps.append(copy.deepcopy(_metadata_step(release, AUTHORIZATION_STEP)))
    violations: list[str] = []

    check_authorization_wiring(release, violations)

    assert any(v.startswith("authorization.step") for v in violations), (
        f"a duplicated step id must be reported: {violations}"
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
