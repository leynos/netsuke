"""Hold which job executes tests, and stop coverage being measured twice.

A coverage job and a test-only job are worth running together only when the
test-only job executes something the coverage job does not. That is the case
here on a pull request and is not the case on the trunk, so the two are
separated by event rather than merged. These checks pin that split, and pin
the flags that make the merge gate's suite a strict superset of the coverage
run, so a later edit cannot quietly narrow the gate into a duplicate.

Run via ``make test-workflow-contracts``.
"""

import pytest
from cache_contract_data import WORKFLOW_DIR
from workflow_loading import (
    MAKEFILE_PATH,
    job_steps,
    load_workflow,
    named_step,
)

#: Flags that make the merge gate's suite broader than the coverage run.
#: `generate-coverage` invokes `cargo llvm-cov nextest --workspace` with
#: default features and default targets, so without these the gate would stop
#: exercising the `legacy-digests` feature and the bench targets.
REQUIRED_NEXTEST_FLAGS = ("--workspace", "--all-targets", "--all-features")
#: `cargo llvm-cov nextest` cannot run doctests, so the gate must run them.
REQUIRED_DOCTEST_FLAGS = ("--workspace", "--doc", "--all-features")

#: Every job that measures coverage, and the event it is restricted to. Two
#: coverage runs against one commit pay twice and give the ratchet baseline
#: two writers.
COVERAGE_PRODUCERS = {
    ("ci.yml", "build-test"): "github.event_name == 'pull_request'",
    ("coverage-main.yml", "coverage-upload"): None,
}


def _makefile_recipe(target: str) -> str:
    """Return the recipe lines of a Makefile target."""
    lines = MAKEFILE_PATH.read_text(encoding="utf-8").splitlines()
    start = next(
        index
        for index, line in enumerate(lines)
        if line.startswith(f"{target}:") or line.startswith(f"{target} ")
    )
    recipe: list[str] = []
    for line in lines[start + 1 :]:
        if line.startswith("\t"):
            recipe.append(line)
        elif line.strip():
            break
    return "\n".join(recipe)


def test_the_merge_gate_runs_the_full_workspace_suite() -> None:
    """Require the surviving Linux test gate to execute every test.

    The gate is the only job that runs the workspace suite on Linux, so the
    breadth of its invocation is the contract: the whole workspace, every
    target, and every feature, with warnings denied.
    """
    steps = job_steps(load_workflow(WORKFLOW_DIR / "ci.yml"), "build-test")
    assert named_step(steps, "Test").get("run") == "make test", (
        "the merge gate must run the canonical test target"
    )
    nextest = _makefile_recipe("test-nextest")
    for flag in REQUIRED_NEXTEST_FLAGS:
        assert flag in nextest, f"the nextest suite must pass {flag}"
    assert "-D warnings" in nextest, "the nextest suite must deny warnings"


def test_the_merge_gate_runs_doctests_the_coverage_run_cannot() -> None:
    """Require doctests, which `cargo llvm-cov nextest` never executes."""
    doctest = _makefile_recipe("doctest")
    for flag in REQUIRED_DOCTEST_FLAGS:
        assert flag in doctest, f"the doctest pass must pass {flag}"
    assert "test: test-nextest doctest" in MAKEFILE_PATH.read_text(encoding="utf-8"), (
        "`make test` must compose the nextest and doctest passes"
    )


@pytest.mark.parametrize(
    ("workflow_name", "job_name", "expected_condition"),
    [
        (workflow, job, condition)
        for (workflow, job), condition in COVERAGE_PRODUCERS.items()
    ],
)
def test_one_coverage_producer_per_event(
    workflow_name: str, job_name: str, expected_condition: str | None
) -> None:
    """Keep exactly one job measuring coverage for any given commit.

    The merge gate measures coverage only on a pull request, where it feeds
    the changed-line gate. On the trunk `coverage-upload` measures the same
    commit and uploads it, and is the sole writer of the ratchet baseline.
    """
    workflow = load_workflow(WORKFLOW_DIR / workflow_name)
    step = named_step(job_steps(workflow, job_name), "Test and Measure Coverage")
    assert step.get("if") == expected_condition, (
        f"{workflow_name} {job_name} must measure coverage under "
        f"{expected_condition!r}, got {step.get('if')!r}"
    )


def test_the_trunk_coverage_job_is_the_only_baseline_writer() -> None:
    """Require the trunk coverage job to own the ratchet baseline alone."""
    trunk = load_workflow(WORKFLOW_DIR / "coverage-main.yml")
    triggers = trunk.get("on")
    assert isinstance(triggers, dict), "coverage-main.yml must declare triggers"
    assert set(triggers) == {"push"}, (
        f"coverage-main.yml must run only on a push, got {sorted(triggers)!r}"
    )
