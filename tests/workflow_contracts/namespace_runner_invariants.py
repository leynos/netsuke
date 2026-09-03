"""Define pure validation rules for Netsuke's Namespace runner workflows.

These helpers are owned by the workflow-contract suites. They accept normalized
step or runner-assignment data, perform no YAML input/output, and should not be
used by production code. Checked-in workflow tests and generated property tests
share them so both forms enforce one contract.
"""

NINJA_ACTION_REPOSITORY = "seanmiddleditch/gha-setup-ninja@"
NINJA_ACTION = (
    "seanmiddleditch/gha-setup-ninja@3b1f8f94a2f8254bd26914c4ab9474d4f0015f67"
)
WINDOWS_PATH_STEP_NAME = "Expose Windows global tool path"
WINDOWS_PACKAGE_ACTION = "leynos/shared-actions/.github/actions/windows-package@"
CALLER_SELECTED_RUNNER = "${{ inputs.runner }}"
CALLEE_SELECTED_RUNNER = "<callee-selected>"

WINDOWS_PATH_FRAGMENTS = (
    "[Environment]::GetFolderPath([Environment+SpecialFolder]::UserProfile)",
    "'.dotnet'",
    "'tools'",
    "$Env:GITHUB_PATH",
)

REQUIRED_RUNNER_ASSIGNMENTS = {
    "ci.build-test": "namespace-profile-netsuke-ci",
    "ci-windows.build-test-windows": "namespace-profile-netsuke-windows-ci",
    "ci-windows.windows-native-recipe-smoke": "namespace-profile-netsuke-windows",
    "ci.kani-smoke": "namespace-profile-netsuke",
    "coverage-main.coverage-upload": "namespace-profile-netsuke",
    "delayed-pr-comment.delay_and_comment": "ubuntu-latest",
    "netsukefile-test.netsukefile": "namespace-profile-netsuke-ubuntu-22-04",
    "release.metadata": "namespace-profile-netsuke",
    "release.build-linux": "namespace-profile-netsuke",
    "release.build-windows": "namespace-profile-netsuke-windows",
    "release.macos.x86_64-apple-darwin": "macos-15-intel",
    "release.macos.aarch64-apple-darwin": ("namespace-profile-netsuke-macos-arm64"),
    "release.windows-native-recipe-smoke": "namespace-profile-netsuke-windows",
    "release.release": "namespace-profile-netsuke",
    "build-and-package.build": CALLER_SELECTED_RUNNER,
    "mutation-testing.mutation": CALLEE_SELECTED_RUNNER,
    "dependabot-automerge.automerge": CALLEE_SELECTED_RUNNER,
}

NAMESPACE_ASSIGNMENT_KEYS = tuple(
    key
    for key, runner in REQUIRED_RUNNER_ASSIGNMENTS.items()
    if runner.startswith("namespace-profile-")
)


def is_valid_ninja_sequence(
    steps: list[dict[str, object]], first_consumer: str
) -> bool:
    """Return whether one pinned Ninja setup precedes the first consumer."""
    setup_indices = [
        index
        for index, step in enumerate(steps)
        if str(step.get("uses", "")).startswith(NINJA_ACTION_REPOSITORY)
    ]
    consumer_indices = [
        index for index, step in enumerate(steps) if step.get("name") == first_consumer
    ]
    return bool(
        len(setup_indices) == 1
        and steps[setup_indices[0]].get("uses") == NINJA_ACTION
        and consumer_indices
        and setup_indices[0] < min(consumer_indices)
    )


def is_valid_windows_tool_path_sequence(
    steps: list[dict[str, object]],
) -> bool:
    """Return whether one guarded PowerShell path setup precedes packaging."""
    path_indices = [
        index for index, step in enumerate(steps) if _is_windows_path_candidate(step)
    ]
    package_indices = [
        index
        for index, step in enumerate(steps)
        if str(step.get("uses", "")).startswith(WINDOWS_PACKAGE_ACTION)
    ]
    if len(path_indices) != 1 or len(package_indices) != 1:
        return False

    path_index = path_indices[0]
    return _is_valid_windows_path_step(steps[path_index]) and (
        path_index < package_indices[0]
    )


def has_required_runner_assignments(assignments: dict[str, str]) -> bool:
    """Return whether every owned and delegated runner keeps its assignment."""
    return dict(assignments) == REQUIRED_RUNNER_ASSIGNMENTS


def _is_windows_path_candidate(step: dict[str, object]) -> bool:
    """Identify a step that appears to configure the Windows tool path."""
    script = str(step.get("run", ""))
    return bool(
        step.get("name") == WINDOWS_PATH_STEP_NAME
        or (
            "$Env:GITHUB_PATH" in script
            and ("SpecialFolder" in script or ".dotnet" in script)
        )
    )


def _is_valid_windows_path_step(step: dict[str, object]) -> bool:
    """Validate the guarded PowerShell known-folder path setup itself."""
    script = str(step.get("run", ""))
    return bool(
        step.get("name") == WINDOWS_PATH_STEP_NAME
        and step.get("if") == "inputs.platform == 'windows'"
        and step.get("shell") == "pwsh"
        and all(fragment in script for fragment in WINDOWS_PATH_FRAGMENTS)
    )
