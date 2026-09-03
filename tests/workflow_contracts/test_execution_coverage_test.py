"""Hold the single-execution rule for Linux tests.

One instrumented run measures coverage and executes the suite together, so the
lane compiles the workspace once per pull request instead of twice. That only
stays honest while the instrumented invocation is as broad as an
uninstrumented one would have been, so these checks pin its flags, pin the
doctest pass that `cargo llvm-cov nextest` cannot perform, and reject a second
Linux job quietly reintroducing `cargo nextest` or `cargo test`.

Run via ``make test-workflow-contracts``.
"""

import re

import pytest
from cache_contract_data import WORKFLOW_DIR
from workflow_loading import (
    MAKEFILE_PATH,
    job_steps,
    load_workflow,
    named_step,
)

#: Inputs that make the instrumented run as broad as `make test` was. Without
#: `all-features` the `legacy-digests` tests in `src/stdlib/path/hash_utils.rs`
#: and `tests/std_filter_tests/hash_filters.rs` stop running; without
#: `all-targets` the two `benches/` targets stop compiling.
REQUIRED_COVERAGE_INPUTS = {
    "all-features": "true",
    "all-targets": "true",
    "doctests": "false",
}

#: Every job that measures coverage, and the event it is restricted to. Two
#: coverage runs against one commit pay twice and give the ratchet baseline
#: two writers.
COVERAGE_PRODUCERS = {
    ("ci.yml", "build-test"): "github.event_name == 'pull_request'",
    ("coverage-main.yml", "coverage-upload"): None,
}

#: Linux jobs allowed to execute Rust tests outside the instrumented run.
#: `netsukefile` builds a manifest and drives Ninja on Ubuntu 22.04, and
#: `kani-smoke` runs verification harnesses; neither is a unit-test lane.
LINUX_TEST_EXEMPTIONS = {
    ("ci.yml", "kani-smoke"),
    ("netsukefile-test.yml", "netsukefile"),
}
LINUX_WORKFLOWS = ("ci.yml", "coverage-main.yml", "netsukefile-test.yml")
#: Patterns for a Rust suite execution. `make test` is matched only as a whole
#: target name, so the unrelated `make test-markdown-format` and
#: `make test-workflow-contracts` gates are not mistaken for one.
FORBIDDEN_TEST_COMMANDS = (
    re.compile(r"\bcargo nextest\b"),
    re.compile(r"\bcargo test\b"),
    re.compile(r"\bmake test(?![\w-])"),
)


def _makefile_recipe(target: str) -> str:
    """Return the recipe lines of a Makefile target.

    Parameters
    ----------
    target
        Makefile target whose recipe is wanted.

    Returns
    -------
    str
        The recipe's lines, joined by newlines.
    """
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


@pytest.mark.parametrize(
    ("workflow_name", "job_name"), sorted(COVERAGE_PRODUCERS, key=str)
)
def test_the_instrumented_run_is_as_broad_as_an_uninstrumented_one(
    workflow_name: str, job_name: str
) -> None:
    """Require every coverage run to execute the whole suite.

    The instrumented run replaced a separate `cargo nextest` execution, so
    narrowing its features or targets would silently retire tests rather than
    remove duplicated work.
    """
    workflow = load_workflow(WORKFLOW_DIR / workflow_name)
    step = named_step(job_steps(workflow, job_name), "Test and Measure Coverage")
    inputs = step.get("with")
    assert isinstance(inputs, dict), f"{workflow_name} {job_name} must pass inputs"
    for name, expected in REQUIRED_COVERAGE_INPUTS.items():
        assert inputs.get(name) == expected, (
            f"{workflow_name} {job_name} must pass {name}={expected!r}, "
            f"got {inputs.get(name)!r}"
        )


def test_warnings_are_denied_through_the_toolchain_setup() -> None:
    """Require `-D warnings` to reach the instrumented run.

    The job declares no `env.RUSTFLAGS`; `tests/polonius_toolchain_contract.rs`
    holds that. `setup-rust` exports the flag instead, and `cargo llvm-cov`
    appends its instrumentation to whatever it finds, so the setting survives.
    """
    for workflow_name, job_name in COVERAGE_PRODUCERS:
        workflow = load_workflow(WORKFLOW_DIR / workflow_name)
        setup = named_step(job_steps(workflow, job_name), "Setup Rust")
        inputs = setup.get("with")
        assert isinstance(inputs, dict), "Setup Rust must declare inputs"
        assert inputs.get("rustflags") == "-D warnings", (
            f"{workflow_name} {job_name} must deny warnings through setup-rust"
        )


def test_a_doctest_pass_follows_the_instrumented_run() -> None:
    """Require the doctests that `cargo llvm-cov nextest` cannot execute."""
    steps = job_steps(load_workflow(WORKFLOW_DIR / "ci.yml"), "build-test")
    doctests = named_step(steps, "Doctests")
    assert doctests.get("run") == "make doctest", (
        "the doctest pass must run the canonical make target"
    )
    coverage_index = steps.index(named_step(steps, "Test and Measure Coverage"))
    assert coverage_index < steps.index(doctests), (
        "the doctest pass must follow the instrumented run"
    )
    assert doctests.get("if") is None, (
        "doctests must run on every event, because no coverage run executes them"
    )
    recipe = _makefile_recipe("doctest")
    for flag in ("--workspace", "--doc", "--all-features"):
        assert flag in recipe, f"the doctest pass must pass {flag}"
    assert "-D warnings" in recipe, "the doctest pass must deny warnings"


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
    """Keep exactly one job measuring coverage for any given commit."""
    workflow = load_workflow(WORKFLOW_DIR / workflow_name)
    step = named_step(job_steps(workflow, job_name), "Test and Measure Coverage")
    assert step.get("if") == expected_condition, (
        f"{workflow_name} {job_name} must measure coverage under "
        f"{expected_condition!r}, got {step.get('if')!r}"
    )


def test_no_other_linux_job_executes_the_rust_suite() -> None:
    """Reject a second Linux job running the workspace suite.

    A duplicate execution is exactly what folding the gate into the coverage
    run removed, so it must not reappear under another step name.
    """
    offenders: list[str] = []
    for workflow_name in LINUX_WORKFLOWS:
        workflow = load_workflow(WORKFLOW_DIR / workflow_name)
        jobs = workflow.get("jobs")
        assert isinstance(jobs, dict), f"{workflow_name} must declare jobs"
        for job_name, declaration in jobs.items():
            if (workflow_name, job_name) in LINUX_TEST_EXEMPTIONS:
                continue
            if not isinstance(declaration, dict) or "steps" not in declaration:
                # A job that calls a reusable workflow declares no steps; the
                # callee's own contracts cover it.
                continue
            for step in job_steps(workflow, job_name):
                script = str(step.get("run", ""))
                offenders += [
                    f"{workflow_name} {job_name} {step.get('name')!r}: "
                    f"{pattern.pattern}"
                    for pattern in FORBIDDEN_TEST_COMMANDS
                    if pattern.search(script)
                ]
    assert not offenders, f"a second Linux test execution reappeared: {offenders!r}"
