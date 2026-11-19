"""Prove dry runs reach release staging without publishing.

The release-path contract in :mod:`release_publish_path_contract` runs the
whole scenario table, so it reports a broken guard, a dropped dependency, or a
widened grant. What a table-driven run cannot show is that the checks were
given the run the table meant: a scenario is checked against results the graph
propagates, not against the outcomes it happens to name. These cases pin the
pieces a green table run would otherwise leave implicit -- the scenario table's
own refusal of an unmodelled dependency, the verifier path's normalisation, and
the consumer-smoke oracle reading the resolved results -- so a regression in
one of them fails here rather than passing as agreement with itself.

Run via ``make test-workflow-contracts``.
"""

import pytest
from action_run_sources import action_sources
from release_admission_gate_contract import (
    CHECKOUT_ACTION,
    GATE_SCRIPT,
    UV_ISOLATION_FLAGS,
    verifier_gate_command,
)
from release_publish_path_consumer_smoke import (
    CONSUMER_SMOKE_ACTION_PATH,
    ConsumerSmokeCheck,
    check_consumer_smoke,
    smoke_job,
)
from release_publish_path_contract import (
    CALLER_WORKFLOW_PATH,
    check_release_publish_path,
)
from release_publish_path_metadata import metadata_outputs
from release_publish_path_results import resolve_results
from release_publish_path_scenario_table import SCENARIOS
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
    steps: list[dict[str, object]] = [
        {"uses": f"{CHECKOUT_ACTION}@v4", "with": {"path": "./gate"}}
    ]
    assert verifier_gate_command(steps) == (
        f"uv run {UV_ISOLATION_FLAGS} --script gate/{GATE_SCRIPT}"
    ), "a leading './' in the checkout path must not change the gate command"


def test_the_smoke_oracle_reads_the_results_the_graph_resolves() -> None:
    """Evaluate the smoke against propagated results, not declared outcomes.

    ``publish-build-linux-failure`` declares every one of the smoke's
    dependencies successful and fails only the build. The graph cannot produce
    that: with the build failed, inventory verification and finalization are
    skipped, so the smoke must not run. The job's guard is replaced with one
    that reads no dependency, so only the oracle's expected value is observed.
    An oracle reading the declared map still expects the smoke to run and
    reports nothing, which is the agreement-with-itself this case rules out.
    """
    release = load_workflow(RELEASE_WORKFLOW_PATH)
    violations: list[str] = []
    outputs = metadata_outputs(release, "publish", violations)
    scenario = next(
        item for item in SCENARIOS if item.name == "publish-build-linux-failure"
    )
    run = resolve_results(release, scenario, outputs, violations)
    assert scenario.needs_results["finalize-github-release"] == "success", (
        "the scenario must declare the smoke's dependency successful for this "
        "case to tell the two oracles apart"
    )
    assert run.results["finalize-github-release"] == "skipped", (
        "the graph must resolve the failed build's dependants as skipped, got "
        f"{run.results['finalize-github-release']!r}"
    )
    smoke_job(release, violations)["if"] = "!cancelled()"
    check_consumer_smoke(
        ConsumerSmokeCheck(
            release=release,
            action=action_sources().get(CONSUMER_SMOKE_ACTION_PATH),
            scenario=scenario,
            outputs=outputs,
            results=run.results,
            violations=violations,
        )
    )
    assert any(
        "consumer-smoke-job" in violation and "expected False" in violation
        for violation in violations
    ), f"the smoke must be reported as unreachable, got {violations!r}"


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
