"""Prove the scenario model derives results a release run could produce.

The scenario table exists to evaluate guards, so a scenario that names an
impossible combination of results evaluates a guard against a run that cannot
happen. These cases pin the derivation that prevents it: a failed build leaves
every publisher and the verifier skipped, a failure a scenario does declare
survives when its own prerequisites succeeded, and the rehearsal keeps running
where it does not depend on the failure.
"""

import pytest
from release_publish_path_metadata import metadata_outputs
from release_publish_path_results import dependency_order, resolve_results
from release_publish_path_scenarios import (
    GITHUB_SHA,
    Scenario,
    _needs_results,
)
from workflow_loading import RELEASE_WORKFLOW_PATH, load_workflow


@pytest.fixture(scope="module")
def release() -> dict[str, object]:
    """Parse the release workflow once for the derivation cases."""
    return load_workflow(RELEASE_WORKFLOW_PATH)


def _scenario(mode: str, outcomes: dict[str, str] | None = None) -> Scenario:
    """Build one scenario whose only declared outcomes are *outcomes*."""
    return Scenario(
        f"{mode}-derivation",
        mode,
        "opened",
        _needs_results(outcomes=outcomes),
        cancelled=False,
        release_runs=False,
    )


def _outputs(release: dict[str, object], mode: str) -> dict[str, str]:
    """Resolve the metadata outputs the guards read for *mode*."""
    violations: list[str] = []
    outputs = metadata_outputs(release, mode, violations)
    assert not violations, f"metadata outputs should resolve; got {violations!r}"
    return outputs


def test_a_failed_build_leaves_every_dependent_job_skipped(
    release: dict[str, object],
) -> None:
    """Refuse to model a publisher succeeding when the build it publishes failed."""
    violations: list[str] = []

    run = resolve_results(
        release,
        _scenario("publish", {"build-linux": "failure"}),
        _outputs(release, "publish"),
        violations,
    )
    results = run.results

    assert results["build-linux"] == "failure", "the declared outcome must survive"
    for name in (
        "publish-crates-io",
        "publish-release",
        "verify-release-inventory",
        "finalize-github-release",
    ):
        assert results[name] == "skipped", (
            f"{name} cannot succeed once build-linux failed; got {results[name]!r}"
        )
    assert not violations, f"derivation should report nothing, got {violations!r}"


def test_a_declared_failure_survives_when_its_own_needs_succeeded(
    release: dict[str, object],
) -> None:
    """Keep a publisher failure the case exists to exercise."""
    results = resolve_results(
        release,
        _scenario("publish", {"publish-crates-io": "failure"}),
        _outputs(release, "publish"),
        [],
    ).results

    assert results["publish-crates-io"] == "failure", (
        "a publisher whose own prerequisites succeeded can still fail"
    )
    assert results["finalize-github-release"] == "skipped", (
        "finalization cannot run once a publisher failed"
    )


def test_the_rehearsal_outlives_a_failure_it_does_not_need(
    release: dict[str, object],
) -> None:
    """Keep the read-only lane reachable when packaging fails on a dry run."""
    results = resolve_results(
        release,
        _scenario("dry-run", {"package-crate": "failure"}),
        _outputs(release, "dry-run"),
        [],
    ).results

    assert results["release"] == "success", (
        "the rehearsal does not depend on packaging and must still run"
    )
    assert results["publish-release"] == "skipped", (
        "a rehearsal never reaches publication"
    )


def test_a_dependency_cycle_is_reported_rather_than_looped(
    release: dict[str, object],
) -> None:
    """Report a graph that cannot be ordered, naming every job in it."""
    jobs = {
        "first": {"needs": ["second"]},
        "second": {"needs": ["first"]},
    }
    violations: list[str] = []

    order = dependency_order(jobs, violations)

    assert sorted(order) == ["first", "second"], (
        "an unorderable graph should still name all its jobs"
    )
    assert violations == ["release.jobs: dependency cycle among ['first', 'second']"], (
        f"the cycle should be reported once, got {violations!r}"
    )


def test_a_scenario_models_its_candidate_as_the_run_candidate(
    release: dict[str, object],
) -> None:
    """Keep the shared candidate constant as the one every guard compares."""
    assert len(GITHUB_SHA) == 40, "the modelled candidate must be a full commit id"
