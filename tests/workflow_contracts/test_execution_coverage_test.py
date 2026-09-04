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
    require_mapping,
    workflow_job,
)

#: Inputs that make the coverage run as broad as `make test` was. Without
#: `all-features` the `legacy-digests` tests in `src/stdlib/path/hash_utils.rs`
#: and `tests/std_filter_tests/hash_filters.rs` stop running; without
#: `all-targets` the two `benches/` targets stop compiling; without `doctests`
#: nothing runs the doctests, which `cargo llvm-cov nextest` cannot execute.
REQUIRED_COVERAGE_INPUTS = {
    "all-features": "true",
    "all-targets": "true",
    "doctests": "true",
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
        (
            index
            for index, line in enumerate(lines)
            if line.startswith(f"{target}:") or line.startswith(f"{target} ")
        ),
        None,
    )
    if start is None:
        pytest.fail(f"the Makefile must declare a {target!r} target")
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


@pytest.mark.parametrize(
    ("workflow_name", "job_name"), sorted(COVERAGE_PRODUCERS, key=str)
)
def test_warnings_are_denied_through_the_toolchain_setup(
    workflow_name: str, job_name: str
) -> None:
    """Require `-D warnings` to reach the instrumented run.

    The job declares no `env.RUSTFLAGS`; `tests/polonius_toolchain_contract.rs`
    holds that. `setup-rust` exports the flag instead, and `cargo llvm-cov`
    appends its instrumentation to whatever it finds, so the setting survives.
    """
    workflow = load_workflow(WORKFLOW_DIR / workflow_name)
    setup = named_step(job_steps(workflow, job_name), "Setup Rust")
    inputs = setup.get("with")
    assert isinstance(inputs, dict), "Setup Rust must declare inputs"
    assert inputs.get("rustflags") == "-D warnings", (
        f"{workflow_name} {job_name} must deny warnings through setup-rust"
    )


def test_no_bespoke_doctest_step_shadows_the_action() -> None:
    """Require the doctest pass to come from the coverage action.

    The action runs `cargo test --doc --workspace` under the same feature
    selection as the instrumented run, so a hand-rolled step beside it would
    state that selection twice and let the two drift apart.
    """
    steps = job_steps(load_workflow(WORKFLOW_DIR / "ci.yml"), "build-test")
    assert not [step for step in steps if step.get("name") == "Doctests"], (
        "the coverage action's doctests input replaced the bespoke step"
    )


def test_the_local_test_target_still_runs_both_passes() -> None:
    """Keep `make test` running what CI runs, for local parity.

    CI drives the doctests through the coverage action, but a contributor runs
    `make test`, so the target must still compose both passes with the same
    breadth.
    """
    makefile = MAKEFILE_PATH.read_text(encoding="utf-8")
    assert "test: test-nextest doctest" in makefile, (
        "`make test` must compose the nextest and doctest passes"
    )
    nextest = _makefile_recipe("test-nextest")
    for flag in ("--workspace", "--all-targets", "--all-features"):
        assert flag in nextest, f"the local nextest pass must pass {flag}"
    doctest = _makefile_recipe("doctest")
    for flag in ("--workspace", "--doc", "--all-features"):
        assert flag in doctest, f"the local doctest pass must pass {flag}"
    for recipe, label in ((nextest, "nextest"), (doctest, "doctest")):
        assert "-D warnings" in recipe, f"the local {label} pass must deny warnings"


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


def _step_scan_texts(step: dict[str, object]) -> list[tuple[str, str]]:
    """Return every scannable command text on a step, paired with its source.

    A Linux job can reintroduce a suite execution through a bare `run`
    script, or through a composite action input such as `with.args`, so
    both the `run` script and every `with` value are scanned with the same
    forbidden-command patterns.

    Parameters
    ----------
    step
        A single workflow step.

    Returns
    -------
    list[tuple[str, str]]
        Pairs of source label and text. The label is ``"run"`` for the step's
        script and ``"with.<key>"`` for each of its inputs, so a failure names
        where the forbidden command was found.
    """
    texts = [("run", str(step.get("run", "")))]
    with_inputs = step.get("with")
    if isinstance(with_inputs, dict):
        texts += [(f"with.{key}", str(value)) for key, value in with_inputs.items()]
    return texts


def _step_offenders(step: dict[str, object]) -> list[str]:
    """Return one label per forbidden pattern this step's texts match."""
    return [
        f"{step.get('name')!r} ({source}): {pattern.pattern}"
        for source, text in _step_scan_texts(step)
        for pattern in FORBIDDEN_TEST_COMMANDS
        if pattern.search(text)
    ]


def _is_scannable_job(workflow_name: str, job_name: str, declaration: object) -> bool:
    """Return whether a job declares steps this contract should scan."""
    if (workflow_name, job_name) in LINUX_TEST_EXEMPTIONS:
        return False
    # A job that calls a reusable workflow declares no steps of its own; the
    # callee's own contracts cover it.
    match declaration:
        case {"steps": _}:
            return True
        case _:
            return False


def _workflow_offenders(workflow_name: str) -> list[str]:
    """Return every forbidden-command match in one workflow's Linux jobs."""
    workflow = load_workflow(WORKFLOW_DIR / workflow_name)
    jobs = workflow.get("jobs")
    assert isinstance(jobs, dict), f"{workflow_name} must declare jobs"
    return [
        f"{workflow_name} {job_name} {offender}"
        for job_name, declaration in jobs.items()
        if _is_scannable_job(workflow_name, job_name, declaration)
        for step in job_steps(workflow, job_name)
        for offender in _step_offenders(step)
    ]


def test_no_other_linux_job_executes_the_rust_suite() -> None:
    """Reject a second Linux job running the workspace suite.

    A duplicate execution is exactly what folding the gate into the coverage
    run removed, so it must not reappear under another step name. The scan
    covers both a step's `run` script and its `with` input values, so a
    composite or shared action wrapping `cargo nextest` cannot reintroduce
    the suite either.
    """
    offenders = [
        offender
        for workflow_name in LINUX_WORKFLOWS
        for offender in _workflow_offenders(workflow_name)
    ]
    assert not offenders, f"a second Linux test execution reappeared: {offenders!r}"


#: The instrumented lanes' budget for one `cargo llvm-cov nextest` invocation,
#: in seconds. The shared coverage action defaults to 600, which was sized
#: against a lane that restored a `target` archive and so never paid for a cold
#: compile.
CARGO_WAIT_TIMEOUT = "1800"


@pytest.mark.parametrize(
    ("workflow_name", "job_name"), sorted(COVERAGE_PRODUCERS, key=str)
)
def test_instrumented_lanes_budget_for_a_cold_build(
    workflow_name: str, job_name: str
) -> None:
    """Require both instrumented lanes to raise the cargo watchdog.

    The shared coverage action wraps `cargo llvm-cov nextest` in a watchdog
    that defaults to 600 seconds. That budget assumed a restored `target`
    archive; this repository archives no build tree, so a cold sccache store
    leaves the whole instrumented build to do inside it. The first trunk run
    after the runner migration failed exactly there: all 2,790 tests passed,
    taking about 512 seconds at 19.42% sccache hits, and the watchdog killed
    cargo 88 seconds later during report generation.

    Held for both producers, not only the lane that failed. The merge gate
    runs the same action on pull requests and meets the same wall whenever its
    store is cold, which is the case a green trunk run would otherwise hide.
    """
    job = workflow_job(load_workflow(WORKFLOW_DIR / workflow_name), job_name)
    env = require_mapping(job.get("env"), f"{job_name} env")
    assert env.get("RUN_RUST_CARGO_WAIT_TIMEOUT") == CARGO_WAIT_TIMEOUT, (
        f"{workflow_name} {job_name} must budget "
        f"{CARGO_WAIT_TIMEOUT}s for one instrumented cargo run, got "
        f"{env.get('RUN_RUST_CARGO_WAIT_TIMEOUT')!r}"
    )
