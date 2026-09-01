"""Hold Netsuke's Namespace runner-profile assignments.

The repository owns direct Linux, Windows, and ARM64 macOS job placement,
while Intel macOS jobs and externally defined reusable workflows retain their
existing runner ownership. These contracts prevent an unrelated workflow edit
from silently moving repository-owned work back to GitHub-hosted runners.

Run via ``make test-workflow-contracts``.
"""

import pytest
from workflow_loading import REPO_ROOT, load_workflow, require_mapping, workflow_job

WORKFLOW_DIR = REPO_ROOT / ".github" / "workflows"


@pytest.mark.parametrize(
    ("workflow_name", "job_name", "expected_runner"),
    [
        ("ci.yml", "build-test", "namespace-profile-netsuke-ci"),
        (
            "ci.yml",
            "build-test-windows",
            "namespace-profile-netsuke-windows-ci",
        ),
        (
            "ci.yml",
            "windows-native-recipe-smoke",
            "namespace-profile-netsuke-windows",
        ),
        ("ci.yml", "kani-smoke", "namespace-profile-netsuke"),
        ("coverage-main.yml", "coverage-upload", "namespace-profile-netsuke"),
        (
            "delayed-pr-comment.yml",
            "delay_and_comment",
            "namespace-profile-netsuke",
        ),
        (
            "netsukefile-test.yml",
            "netsukefile",
            "namespace-profile-netsuke-ubuntu-22-04",
        ),
        ("release.yml", "metadata", "namespace-profile-netsuke"),
        (
            "release.yml",
            "windows-native-recipe-smoke",
            "namespace-profile-netsuke-windows",
        ),
        ("release.yml", "release", "namespace-profile-netsuke"),
    ],
)
def test_repository_owned_jobs_use_namespace_profiles(
    workflow_name: str,
    job_name: str,
    expected_runner: str,
) -> None:
    """Require each repository-owned job to use its Namespace profile."""
    workflow = load_workflow(WORKFLOW_DIR / workflow_name)
    actual_runner = workflow_job(workflow, job_name).get("runs-on")
    assert actual_runner == expected_runner, (
        f"{workflow_name} job {job_name} must run on {expected_runner}, "
        f"got {actual_runner!r}"
    )


def test_linux_release_build_uses_the_general_namespace_profile() -> None:
    """Require Linux packaging to pass the general profile to its workflow."""
    workflow = load_workflow(WORKFLOW_DIR / "release.yml")
    build_linux = workflow_job(workflow, "build-linux")
    inputs = require_mapping(build_linux.get("with"), "jobs.build-linux.with")
    actual_runner = inputs.get("runner")
    assert actual_runner == "namespace-profile-netsuke", (
        "release.yml build-linux must pass namespace-profile-netsuke to the package "
        f"workflow, got {actual_runner!r}"
    )


def test_windows_release_build_uses_the_general_namespace_profile() -> None:
    """Require Windows packaging to pass its general Namespace profile."""
    workflow = load_workflow(WORKFLOW_DIR / "release.yml")
    build_windows = workflow_job(workflow, "build-windows")
    inputs = require_mapping(build_windows.get("with"), "jobs.build-windows.with")
    actual_runner = inputs.get("runner")
    expected_runner = "namespace-profile-netsuke-windows"
    assert actual_runner == expected_runner, (
        f"release.yml build-windows must pass {expected_runner} to the package "
        f"workflow, got {actual_runner!r}"
    )


def test_macos_release_matrix_preserves_native_architectures() -> None:
    """Use Namespace for ARM64 macOS while retaining GitHub's Intel runner."""
    workflow = load_workflow(WORKFLOW_DIR / "release.yml")
    build_macos = workflow_job(workflow, "build-macos")
    strategy = require_mapping(build_macos.get("strategy"), "jobs.build-macos.strategy")
    matrix = require_mapping(strategy.get("matrix"), "jobs.build-macos.strategy.matrix")
    includes = matrix.get("include")
    assert isinstance(includes, list), "the build-macos matrix include must be a list"
    runners_by_target = {
        entry["target"]: entry["runner"]
        for item in includes
        if isinstance(item, dict)
        for entry in [item]
    }
    assert runners_by_target == {
        "x86_64-apple-darwin": "macos-15-intel",
        "aarch64-apple-darwin": "namespace-profile-netsuke-macos-arm64",
    }, f"unexpected macOS runner matrix: {runners_by_target!r}"


def test_package_workflow_runs_on_its_caller_selected_profile() -> None:
    """Keep runner selection at the release caller's platform boundary."""
    workflow = load_workflow(WORKFLOW_DIR / "build-and-package.yml")
    actual_runner = workflow_job(workflow, "build").get("runs-on")
    assert actual_runner == "${{ inputs.runner }}", (
        "build-and-package.yml must use its caller-selected runner input, "
        f"got {actual_runner!r}"
    )
