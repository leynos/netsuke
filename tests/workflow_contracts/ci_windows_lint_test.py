"""Contract tests for the Windows lint job's Whitaker invocation.

`lint-windows` runs formatting, Clippy and Whitaker concurrently with
`build-test-windows`. Whitaker is the largest step in the lane at a 385s
median, and it must run through the installer's PowerShell wrapper rather than
Git Bash, lint both workspace packages, and propagate a failure from either.
Each of those is a separate way the lint gate can quietly stop being a gate.

These live here rather than in ``ci_windows_job_test.py`` to keep both modules
inside the repository's 400-line file limit. Shared parsing helpers live in
``workflow_loading.py``.

Run via ``make test-workflow-contracts``.
"""

import pytest
from workflow_loading import (
    CI_WINDOWS_WORKFLOW_PATH,
    job_steps,
    load_workflow,
    named_step,
)

LINT_JOB = "lint-windows"

#: Required wrapper, configuration, and root-package lint fragments.
WHITAKER_WORKSPACE_FRAGMENTS = (
    (
        "$profileHome = [Environment]::GetFolderPath("
        "[Environment+SpecialFolder]::UserProfile)"
    ),
    "Join-Path $profileHome '.local\\bin\\whitaker.ps1'",
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


@pytest.fixture
def lint_steps() -> list[dict[str, object]]:
    """Return the lint-windows job's steps, in declaration order."""
    return job_steps(load_workflow(CI_WINDOWS_WORKFLOW_PATH), LINT_JOB)


def test_windows_job_installs_whitaker_before_linting(
    lint_steps: list[dict[str, object]],
) -> None:
    """The shared installer must precede the PowerShell Whitaker invocation."""
    step_names = [str(step.get("name", "")) for step in lint_steps]
    install_index = step_names.index("Install Whitaker")
    lint_index = step_names.index("Lint (Whitaker)")
    assert install_index < lint_index, (
        "Install Whitaker must precede Lint (Whitaker) so the PowerShell wrapper "
        f"exists before it is invoked, got step order {step_names!r}"
    )


def test_windows_job_runs_whitaker_through_powershell_wrapper(
    lint_steps: list[dict[str, object]],
) -> None:
    """Assert that Windows runs both Whitaker packages through PowerShell."""
    step_name = "Lint (Whitaker)"
    step = named_step(lint_steps, step_name)
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
