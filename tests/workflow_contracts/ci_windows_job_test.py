"""Contract tests for the CI workflow's Windows merge gate.

``build-test-windows`` lives in the ``ci-windows.yml`` reusable workflow so
``ci.yml`` stays inside the repository's 400-line file limit. It compiles and
exercises the ``#[cfg(windows)]`` tree, so
it must run on a Windows runner, drive the Makefile's POSIX recipes under Git
Bash, keep ``-D warnings`` in force, run the platform-sensitive gates exactly
once, skip the platform-independent doc and audit gates the Linux job already
covers, and block the merge on any failure. Each of those is a separate way
the job can quietly stop being a gate. Shared parsing helpers live in
``workflow_loading.py``.

Run via ``make test-workflow-contracts``.
"""

import pytest
from workflow_loading import (
    CI_WINDOWS_WORKFLOW_PATH,
    CI_WORKFLOW_PATH,
    job_steps,
    load_workflow,
    named_step,
    require_mapping,
    step_runs,
    step_uses,
    workflow_job,
)

WINDOWS_JOB = "build-test-windows"
LINT_JOB = "lint-windows"

#: Both halves of the Windows merge gate. They run concurrently and neither
#: needs the other, so every property that makes one a gate must hold for both.
WINDOWS_JOBS = (LINT_JOB, WINDOWS_JOB)

#: Which job each Git Bash Makefile gate belongs to after the split. Asserted
#: as a mapping rather than a set so a gate silently migrating between jobs,
#: which would put the lints back in series with the tests, fails here.
EXPECTED_GATE_OWNERS = {
    "make SHELL=bash check-fmt": LINT_JOB,
    "make SHELL=bash lint-clippy": LINT_JOB,
    "make SHELL=bash test": WINDOWS_JOB,
}

#: The Git Bash Makefile gates the Windows job must run through the Makefile,
#: so the POSIX recipes execute under Git Bash.
EXPECTED_WINDOWS_BASH_MAKEFILE_GATES = (
    "make SHELL=bash check-fmt",
    "make SHELL=bash lint-clippy",
    "make SHELL=bash test",
)

#: Doc and audit gates already covered on Linux, plus the retired Bash
#: Whitaker gate. Duplicating or restoring them on Windows buys nothing but
#: runtime and can bypass the PowerShell wrapper.
EXCLUDED_WINDOWS_RUNS = (
    "make spelling",
    "make markdownlint",
    "make nixie",
    "make test-workflow-contracts",
    "make SHELL=bash lint-whitaker",
)

#: Linux-only audit actions the Windows job must not invoke.
EXCLUDED_WINDOWS_ACTIONS = (
    "leynos/shared-actions/.github/actions/generate-coverage",
    "leynos/shared-actions/.github/actions/upload-codescene-coverage",
)


def normalise_run(run: object) -> object:
    """Return a stripped command string, preserving non-string step values."""
    match run:
        case str() as command:
            return command.strip()
        case _:
            return run


@pytest.fixture
def windows_job() -> dict[str, object]:
    """Return the build-test-windows job mapping."""
    return workflow_job(load_workflow(CI_WINDOWS_WORKFLOW_PATH), WINDOWS_JOB)


@pytest.fixture
def windows_steps() -> list[dict[str, object]]:
    """Return the build-test-windows job's steps, in declaration order."""
    return job_steps(load_workflow(CI_WINDOWS_WORKFLOW_PATH), WINDOWS_JOB)


@pytest.fixture
def lane_steps() -> list[dict[str, object]]:
    """Return every step of both Windows jobs, lint first."""
    workflow = load_workflow(CI_WINDOWS_WORKFLOW_PATH)
    return [step for name in WINDOWS_JOBS for step in job_steps(workflow, name)]


def test_windows_job_runs_on_a_github_hosted_runner(
    windows_job: dict[str, object],
) -> None:
    """The Windows job must run on a GitHub-hosted Windows runner.

    Ubicloud publishes Ubuntu images only, so `windows-latest` is the durable
    placement for this lane rather than a fallback.
    """
    expected_runner = "windows-latest"
    assert windows_job.get("runs-on") == expected_runner, (
        f"{WINDOWS_JOB} must run on {expected_runner} so the "
        f"#[cfg(windows)] tree is compiled, got {windows_job.get('runs-on')!r}"
    )


def test_windows_job_uses_git_bash_for_recipes(windows_job: dict[str, object]) -> None:
    """The job runs recipes under Git Bash, not cmd.exe.

    The Makefile uses POSIX shell constructs throughout, and GNU Make's
    default recipe shell on Windows is cmd.exe, so the job must default every
    run step to bash.
    """
    defaults = require_mapping(windows_job.get("defaults"), f"{WINDOWS_JOB}.defaults")
    run = require_mapping(defaults.get("run"), f"{WINDOWS_JOB}.defaults.run")
    assert run.get("shell") == "bash", (
        f"{WINDOWS_JOB} must run recipes under Git Bash "
        f"(defaults.run.shell: bash), got {run.get('shell')!r}"
    )


def test_windows_setup_rust_keeps_warnings(
    windows_steps: list[dict[str, object]],
) -> None:
    """The Windows toolchain setup preserves -D warnings.

    The `#[cfg(windows)]` tree must be compiled under `-D warnings` to surface
    findings, so the shared setup-rust action must receive that flag through
    its `rustflags` input. Polonius does not appear here: the pinned nightly
    enables it by default, and restating a `-Zpolonius` directive is exactly
    the fragility that retiring it removed.
    """
    step = named_step(windows_steps, "Setup Rust")
    uses = str(step.get("uses", ""))
    assert "setup-rust" in uses, (
        f"Setup Rust must use the shared setup-rust action, got {uses!r}"
    )
    with_ = require_mapping(step.get("with"), "Setup Rust's with block")
    assert with_.get("toolchain") == "${{ env.NETSUKE_RUST_TOOLCHAIN }}", (
        "Setup Rust must use the pinned NETSUKE_RUST_TOOLCHAIN, "
        f"got {with_.get('toolchain')!r}"
    )
    assert with_.get("rustflags") == "-D warnings", (
        "Setup Rust must pass -D warnings through rustflags so the "
        f"#[cfg(windows)] tree compiles under warnings-as-errors, "
        f"got {with_.get('rustflags')!r}"
    )


def test_windows_lane_runs_check_fmt_lint_and_test(
    lane_steps: list[dict[str, object]],
) -> None:
    """Assert that listed Makefile gates run exactly once across the lane."""
    runs = [normalise_run(run) for run in step_runs(lane_steps)]
    counts = {command: runs.count(command) for command in EXPECTED_GATE_OWNERS}
    assert set(counts.values()) == {1}, (
        f"the Windows lane must run each Git Bash Makefile gate "
        f"{list(EXPECTED_GATE_OWNERS)!r} exactly once across {list(WINDOWS_JOBS)!r}, "
        f"got occurrence counts {counts!r} from run steps: {runs!r}"
    )


def test_windows_lints_and_tests_run_in_separate_concurrent_jobs() -> None:
    """The lint and test gates must sit in different jobs, neither waiting.

    Scenario: 518s of formatting, Clippy and Whitaker used to run in series
    ahead of a 471s test step in one job, measured over the 57 runs between run
    33890685806 and run 34064668331. Invariant: each gate runs in the job that
    owns it and neither job declares `needs`, so putting them back in series,
    or making one wait for the other, fails here rather than quietly costing
    every pull request eight minutes again.
    """
    workflow = load_workflow(CI_WINDOWS_WORKFLOW_PATH)
    owners = {}
    for job_name in WINDOWS_JOBS:
        job = workflow_job(workflow, job_name)
        assert "needs" not in job, (
            f"{job_name} must not wait on another job; the two halves of the "
            f"Windows gate are independent, got needs={job.get('needs')!r}"
        )
        for run in step_runs(job_steps(workflow, job_name)):
            owners[normalise_run(run)] = job_name

    actual = {command: owners.get(command) for command in EXPECTED_GATE_OWNERS}
    assert actual == EXPECTED_GATE_OWNERS, (
        f"each Windows Makefile gate must run in the job that owns it; "
        f"expected {EXPECTED_GATE_OWNERS!r}, got {actual!r}"
    )


def test_windows_lane_does_not_duplicate_doc_and_audit_gates(
    lane_steps: list[dict[str, object]],
) -> None:
    """The Windows job excludes platform-independent doc and audit gates.

    `make spelling`, `make markdownlint`, `make nixie`, coverage generation,
    the CodeScene gate, and `make test-workflow-contracts` are already covered
    on Linux. `make SHELL=bash lint-whitaker` is replaced by the PowerShell
    wrapper. Duplicating or restoring any of them on Windows buys nothing.
    """
    runs = [normalise_run(run) for run in step_runs(lane_steps)]
    duplicated = [command for command in EXCLUDED_WINDOWS_RUNS if command in runs]
    assert not duplicated, (
        f"the Windows lane must not run the platform-independent {duplicated!r}, "
        f"got run steps: {runs!r}"
    )

    uses = step_uses(lane_steps)
    duplicated_actions = [
        action
        for action in EXCLUDED_WINDOWS_ACTIONS
        if any(action in reference for reference in uses)
    ]
    assert not duplicated_actions, (
        f"the Windows lane must not use the Linux-only audit actions "
        f"{duplicated_actions!r}, got action steps: {uses!r}"
    )


def test_windows_job_is_a_blocking_merge_gate(
    windows_job: dict[str, object],
    windows_steps: list[dict[str, object]],
) -> None:
    """No step in the Windows job is allowed to fail silently.

    A `continue-on-error: true` on the job or any step would let a Windows
    lint or test failure pass the merge, defeating the gate.
    """
    assert windows_job.get("continue-on-error") is not True, (
        f"{WINDOWS_JOB} must not set continue-on-error on the job"
    )
    lenient = [
        step.get("name")
        for step in windows_steps
        if step.get("continue-on-error") is True
    ]
    assert not lenient, (
        f"{WINDOWS_JOB} steps {lenient!r} must not set continue-on-error"
    )


#: Version pins the caller repeats as reusable-workflow inputs, mapped to the
#: workflow-level `env` key each one must equal.
CALLER_INPUT_PINS = {
    "nextest-version": "NEXTEST_VERSION",
    "mdtablefix-version": "MDTABLEFIX_VERSION",
    "python-baseline": "PYTHON_BASELINE",
}


def test_windows_caller_repeats_the_workflow_level_pins() -> None:
    """The Windows call must pass the same versions the caller pins itself.

    GitHub does not expose the ``env`` context to a reusable workflow's
    ``with`` block, so the caller has to repeat each literal. Without this
    check the two copies could drift and the Windows gate would silently test
    a different toolchain from the Linux one.
    """
    caller = load_workflow(CI_WORKFLOW_PATH)
    caller_env = require_mapping(caller.get("env"), "the CI workflow env")
    call = workflow_job(caller, "windows")
    assert call.get("uses") == "./.github/workflows/ci-windows.yml", (
        "the Windows gate must be invoked as a local reusable workflow, "
        f"got {call.get('uses')!r}"
    )
    inputs = require_mapping(call.get("with"), "the Windows call inputs")
    for input_name, env_name in CALLER_INPUT_PINS.items():
        assert inputs.get(input_name) == caller_env.get(env_name), (
            f"the Windows call must pass {env_name}'s pinned value as "
            f"{input_name}, got {inputs.get(input_name)!r} against "
            f"{caller_env.get(env_name)!r}"
        )

    called = load_workflow(CI_WINDOWS_WORKFLOW_PATH)
    called_env = require_mapping(called.get("env"), "the Windows workflow env")
    for input_name, env_name in CALLER_INPUT_PINS.items():
        assert called_env.get(env_name) == f"${{{{ inputs['{input_name}'] }}}}", (
            f"the Windows workflow must read {env_name} from its {input_name} "
            f"input, got {called_env.get(env_name)!r}"
        )
