"""Contract tests for the CI workflow's Windows merge gate.

``build-test-windows`` compiles and exercises the ``#[cfg(windows)]`` tree, so
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
    job_steps,
    load_workflow,
    named_step,
    require_mapping,
    step_runs,
    step_uses,
    workflow_job,
)

WINDOWS_JOB = "build-test-windows"

#: The Git Bash Makefile gates the Windows job must run through the Makefile,
#: so the POSIX recipes execute under Git Bash.
EXPECTED_WINDOWS_BASH_MAKEFILE_GATES = (
    "make SHELL=bash check-fmt",
    "make SHELL=bash lint-clippy",
    "make SHELL=bash test",
)

#: Doc and audit gates already covered on Linux; duplicating them on Windows
#: buys nothing but runtime.
EXCLUDED_WINDOWS_RUNS = (
    "make spelling",
    "make markdownlint",
    "make nixie",
    "make test-workflow-contracts",
)

#: Linux-only audit actions the Windows job must not invoke.
EXCLUDED_WINDOWS_ACTIONS = (
    "leynos/shared-actions/.github/actions/generate-coverage",
    "leynos/shared-actions/.github/actions/upload-codescene-coverage",
)

#: Required wrapper, configuration, and root-package lint fragments.
WHITAKER_WORKSPACE_FRAGMENTS = (
    "Join-Path $HOME '.local\\bin\\whitaker.ps1'",
    '$env:RUSTFLAGS = "$env:RUSTFLAGS -D warnings"',
    "$env:DYLINT_TOML = Get-Content dylint.toml -Raw",
    (
        "& $whitaker --all --no-deps --package netsuke-build '--' "
        "--all-targets --all-features"
    ),
    "Push-Location test_support",
)

#: Required nested-package lint and location-restoration fragments.
WHITAKER_TEST_SUPPORT_FRAGMENTS = (
    "$env:DYLINT_TOML = Get-Content dylint.toml -Raw",
    (
        "& $whitaker --all --no-deps --package test_support '--' "
        "--all-targets --all-features"
    ),
    "finally {",
    "Pop-Location",
)

#: Required guard that preserves a native Whitaker failure as the step result.
WHITAKER_EXIT_GUARD_FRAGMENTS = (
    "if ($LASTEXITCODE -ne 0) {",
    "exit $LASTEXITCODE",
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
    return workflow_job(load_workflow(), WINDOWS_JOB)


@pytest.fixture
def windows_steps() -> list[dict[str, object]]:
    """Return the build-test-windows job's steps, in declaration order."""
    return job_steps(load_workflow(), WINDOWS_JOB)


def test_windows_job_runs_on_windows_latest(windows_job: dict[str, object]) -> None:
    """The Windows job must actually run on a Windows runner."""
    assert windows_job.get("runs-on") == "windows-latest", (
        f"{WINDOWS_JOB} must run on windows-latest so the "
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


def test_windows_job_runs_check_fmt_lint_and_test(
    windows_steps: list[dict[str, object]],
) -> None:
    """Assert that listed Makefile gates run exactly once under Git Bash."""
    runs = [normalise_run(run) for run in step_runs(windows_steps)]
    bash_makefile_gates = EXPECTED_WINDOWS_BASH_MAKEFILE_GATES
    counts = {command: runs.count(command) for command in bash_makefile_gates}
    assert set(counts.values()) == {1}, (
        f"{WINDOWS_JOB} must run each Git Bash Makefile gate "
        f"{list(EXPECTED_WINDOWS_BASH_MAKEFILE_GATES)!r} exactly once, got "
        f"occurrence counts {counts!r} from run steps: {runs!r}"
    )


def test_windows_job_runs_whitaker_through_powershell_wrapper(
    windows_steps: list[dict[str, object]],
) -> None:
    """Assert that Windows runs both Whitaker packages through PowerShell."""
    step_name = "Lint (Whitaker)"
    step = named_step(windows_steps, step_name)
    assert step.get("shell") == "pwsh", (
        f"{step_name} must declare the PowerShell Core shell, got {step.get('shell')!r}"
    )
    match step.get("run"):
        case str() as run:
            pass
        case _:
            pytest.fail(f"{step_name} must declare a PowerShell run block")

    missing = [
        fragment for fragment in WHITAKER_WORKSPACE_FRAGMENTS if fragment not in run
    ]
    assert not missing, (
        f"{step_name} must resolve the PowerShell wrapper, append -D warnings, "
        f"load the workspace Dylint configuration, pass the quoted separator to "
        f"lint netsuke-build, and enter "
        f"test_support; missing {missing!r}"
    )

    _, _, run_after_push = run.partition("Push-Location test_support")
    missing = [
        fragment
        for fragment in WHITAKER_TEST_SUPPORT_FRAGMENTS
        if fragment not in run_after_push
    ]
    assert not missing, (
        f"{step_name} must load test_support's Dylint configuration after entering it, "
        f"pass the quoted separator to lint test_support, and restore the location "
        f"in finally; missing "
        f"{missing!r}"
    )

    _, _, run_after_workspace_lint = run.partition(WHITAKER_WORKSPACE_FRAGMENTS[3])
    workspace_exit_guard, _, _ = run_after_workspace_lint.partition(
        "Push-Location test_support"
    )
    missing = [
        fragment
        for fragment in WHITAKER_EXIT_GUARD_FRAGMENTS
        if fragment not in workspace_exit_guard
    ]
    assert not missing, (
        f"{step_name} must stop before entering test_support when linting "
        f"netsuke-build fails; missing {missing!r}"
    )

    _, _, run_after_test_support_lint = run_after_push.partition(
        WHITAKER_TEST_SUPPORT_FRAGMENTS[1]
    )
    test_support_exit_guard, _, _ = run_after_test_support_lint.partition("} finally {")
    missing = [
        fragment
        for fragment in WHITAKER_EXIT_GUARD_FRAGMENTS
        if fragment not in test_support_exit_guard
    ]
    assert not missing, (
        f"{step_name} must propagate a test_support lint failure; missing {missing!r}"
    )


def test_windows_job_does_not_duplicate_doc_and_audit_gates(
    windows_steps: list[dict[str, object]],
) -> None:
    """The Windows job excludes platform-independent doc and audit gates.

    `make spelling`, `make markdownlint`, `make nixie`, coverage generation,
    the CodeScene gate, and `make test-workflow-contracts` are already covered
    on Linux; duplicating them on Windows buys nothing.
    """
    runs = [normalise_run(run) for run in step_runs(windows_steps)]
    duplicated = [command for command in EXCLUDED_WINDOWS_RUNS if command in runs]
    assert not duplicated, (
        f"{WINDOWS_JOB} must not run the platform-independent {duplicated!r}, "
        f"got run steps: {runs!r}"
    )

    uses = step_uses(windows_steps)
    duplicated_actions = [
        action
        for action in EXCLUDED_WINDOWS_ACTIONS
        if any(action in reference for reference in uses)
    ]
    assert not duplicated_actions, (
        f"{WINDOWS_JOB} must not use the Linux-only audit actions "
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
