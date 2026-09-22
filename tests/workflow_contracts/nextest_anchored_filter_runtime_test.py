"""Hold the runtime half of the anchored Nextest filter contract.

`.config/nextest.toml` filters the `nested-cargo-builds` group with
``test(/^NAME($|::)/)`` rather than ``test(=NAME)``, because Nextest's ``=``
form compares a whole test name while a parameterized `#[rstest]` is listed as
``NAME::case_1_…``. A filter in the rejected form parses cleanly and selects
none of the test's instances, so the test silently runs under the default
policy while the configuration still looks enforced.

`nextest_child_cargo_group_invariants` holds the configuration to the anchored
*grammar*, which is all a static read can do. Which tests a filter actually
selects is a question only Nextest can answer, and answering it needs compiled
binaries, so the check itself lives in
``.github/scripts/verify_nextest_anchored_filters.py`` and runs on the coverage
lane. What is asserted here is where it runs and what it must not become.

The second half of this module is the adjacent rule that keeps a *second*
execution of the suite from appearing anywhere in the Linux lanes. Folding
tests and coverage into one instrumented run is what removed the duplicate, and
a listing check placed in the wrong lane is one of the ways it could return.

Run via ``make test-workflow-contracts``.
"""

import re

from cache_contract_data import WORKFLOW_DIR
from workflow_loading import job_steps, load_workflow, named_step

#: The runtime check that holds the filters to the instances Nextest selects.
ANCHORED_FILTER_STEP = "Verify the anchored Nextest filters select their instances"
ANCHORED_FILTER_MODULE = ".github/scripts/verify_nextest_anchored_filters.py"
COVERAGE_STEP = "Test and Measure Coverage"
DISCARD_STEP = "Discard the instrumented build tree"
#: The lane that must not acquire this check. It runs before the first Rust
#: build in the job and is documented as staying static.
STATIC_LANE_STEP = "Workflow contract tests"

#: Linux jobs allowed to execute Rust tests outside the instrumented run.
#: `netsukefile` builds a manifest and drives Ninja on Ubuntu 22.04, and
#: `kani-smoke` runs verification harnesses; neither is a unit-test lane.
LINUX_TEST_EXEMPTIONS = {
    ("ci.yml", "kani-smoke"),
    ("netsukefile-test.yml", "netsukefile"),
}
LINUX_WORKFLOWS = ("ci.yml", "coverage-main.yml", "netsukefile-test.yml")
#: Patterns for a Rust suite execution. `make test` is matched only as a whole
#: target name, so the unrelated `make test-workflow-contracts` gate and its
#: siblings are not mistaken for one.
FORBIDDEN_TEST_COMMANDS = (
    re.compile(r"\bcargo nextest\b"),
    re.compile(r"\bcargo test\b"),
    re.compile(r"\bmake test(?![\w-])"),
)


def test_the_anchored_filter_check_reuses_the_instrumented_run() -> None:
    """Keep the runtime filter check after the coverage run, before the discard.

    The check lists tests, and listing compiles: run it before `Test and
    Measure Coverage` and the lane builds twice. It must also precede the step
    that discards the instrumented tree, which is the only reason its listing
    can reuse the coverage run's compilation rather than paying for a second
    one.
    """
    steps = job_steps(load_workflow(WORKFLOW_DIR / "ci.yml"), "build-test")
    check = named_step(steps, ANCHORED_FILTER_STEP)
    coverage = named_step(steps, COVERAGE_STEP)
    discard = named_step(steps, DISCARD_STEP)
    assert ANCHORED_FILTER_MODULE in str(check.get("run", "")), (
        f"{ANCHORED_FILTER_STEP} must run the checked-in runtime check"
    )
    assert steps.index(coverage) < steps.index(check) < steps.index(discard), (
        f"{ANCHORED_FILTER_STEP} must run after the coverage step, whose "
        f"instrumented tree it reuses, and before {DISCARD_STEP} removes it"
    )


def test_the_anchored_filter_check_stays_off_the_static_lane() -> None:
    """Keep the check out of the lane that runs before the first Rust build.

    `Workflow contract tests` is static by design: it reads workflow files
    rather than executing anything, so it is cheap and it runs before any Rust
    build. A listing check there would make that lane build the workspace, and
    would then build it a second time when the coverage step ran.
    """
    steps = job_steps(load_workflow(WORKFLOW_DIR / "ci.yml"), "build-test")
    static = named_step(steps, STATIC_LANE_STEP)
    check = named_step(steps, ANCHORED_FILTER_STEP)
    assert steps.index(static) < steps.index(check), (
        f"{ANCHORED_FILTER_STEP} must not be folded into {STATIC_LANE_STEP}; it "
        "belongs after the coverage step, not before it"
    )
    assert ANCHORED_FILTER_MODULE not in str(static.get("run", "")), (
        f"{STATIC_LANE_STEP} must stay static: it must not run the listing check"
    )


def test_the_anchored_filter_check_is_gated_with_the_coverage_step() -> None:
    """Gate the check exactly as the coverage step is gated.

    Coverage runs on pull requests only, and the check reuses the tree that run
    produces. On a trunk run the coverage step is skipped, so an ungated check
    would compile the whole workspace by itself -- on the lane whose one
    instrumented run is currently its only build.
    """
    steps = job_steps(load_workflow(WORKFLOW_DIR / "ci.yml"), "build-test")
    check = named_step(steps, ANCHORED_FILTER_STEP)
    coverage = named_step(steps, COVERAGE_STEP)
    assert check.get("if") == coverage.get("if"), (
        f"{ANCHORED_FILTER_STEP} must carry the same condition as {COVERAGE_STEP}: "
        "a run that skipped coverage has no instrumented tree to reuse"
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
