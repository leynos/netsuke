"""Hold Netsuke's Namespace runner-profile assignments.

The repository owns direct Linux, Windows, and ARM64 macOS job placement,
while Intel macOS jobs and externally defined reusable workflows retain their
existing runner ownership. These contracts prevent an unrelated workflow edit
from silently moving repository-owned work back to GitHub-hosted runners.

Run via ``make test-workflow-contracts``.
"""

import pytest
from workflow_loading import (
    REPO_ROOT,
    job_steps,
    load_workflow,
    named_step,
    require_mapping,
    unique_step_index,
    workflow_job,
)

WORKFLOW_DIR = REPO_ROOT / ".github" / "workflows"
NINJA_ACTION = (
    "seanmiddleditch/gha-setup-ninja@3b1f8f94a2f8254bd26914c4ab9474d4f0015f67"
)


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


@pytest.mark.parametrize(
    ("workflow_name", "job_name", "first_consumer"),
    [
        ("ci.yml", "build-test", "Show Ninja version"),
        (
            "netsukefile-test.yml",
            "netsukefile",
            "Build dependent, inline, and foreach targets",
        ),
    ],
)
def test_namespace_linux_jobs_install_ninja_before_use(
    workflow_name: str,
    job_name: str,
    first_consumer: str,
) -> None:
    """Provision pinned Ninja before a Namespace Linux job invokes it."""
    workflow = load_workflow(WORKFLOW_DIR / workflow_name)
    steps = job_steps(workflow, job_name)
    install_step = named_step(steps, "Install Ninja")
    assert install_step.get("uses") == NINJA_ACTION, (
        f"{workflow_name} job {job_name} must install pinned Ninja, "
        f"got {install_step.get('uses')!r}"
    )
    assert unique_step_index(steps, "Install Ninja") < unique_step_index(
        steps, first_consumer
    ), f"{workflow_name} job {job_name} must install Ninja before {first_consumer!r}"


@pytest.mark.parametrize(
    ("job_name", "expected_runner"),
    [
        ("build-linux", "namespace-profile-netsuke"),
        ("build-windows", "namespace-profile-netsuke-windows"),
    ],
)
def test_release_build_uses_the_general_namespace_profile(
    job_name: str,
    expected_runner: str,
) -> None:
    """Require packaging to pass its platform's general Namespace profile."""
    workflow = load_workflow(WORKFLOW_DIR / "release.yml")
    build_job = workflow_job(workflow, job_name)
    inputs = require_mapping(build_job.get("with"), f"jobs.{job_name}.with")
    actual_runner = inputs.get("runner")
    assert actual_runner == expected_runner, (
        f"release.yml {job_name} must pass {expected_runner} to the package "
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


def test_package_workflow_limits_legacy_node_to_windows() -> None:
    """Permit the nested Node 20 action only for Namespace Windows builds."""
    workflow = load_workflow(WORKFLOW_DIR / "build-and-package.yml")
    build = workflow_job(workflow, "build")
    env = require_mapping(build.get("env"), "jobs.build.env")
    expected = "${{ inputs.platform == 'windows' && 'true' || '' }}"
    actual = env.get("ACTIONS_ALLOW_USE_UNSECURE_NODE_VERSION")
    assert actual == expected, (
        "build-and-package.yml must limit the Node 20 compatibility switch "
        f"to Windows, got {actual!r}"
    )


def test_package_workflow_exposes_native_windows_tool_profile() -> None:
    """Expose native global tools before the shared Windows package action."""
    workflow = load_workflow(WORKFLOW_DIR / "build-and-package.yml")
    steps = job_steps(workflow, "build")
    step_name = "Expose Windows global tool path"
    expose_step = named_step(steps, step_name)
    assert expose_step.get("if") == "inputs.platform == 'windows'", (
        f"{step_name} must be limited to Windows packaging"
    )
    assert expose_step.get("shell") == "pwsh", f"{step_name} must use PowerShell"
    script = str(expose_step.get("run", ""))
    required_fragments = (
        "[Environment]::GetFolderPath([Environment+SpecialFolder]::UserProfile)",
        "'.dotnet'",
        "'tools'",
        "$Env:GITHUB_PATH",
    )
    missing = [fragment for fragment in required_fragments if fragment not in script]
    assert not missing, f"{step_name} is missing native profile setup: {missing}"
    assert unique_step_index(steps, step_name) < unique_step_index(
        steps, "Build Windows installer package"
    ), f"{step_name} must run before Windows packaging"
