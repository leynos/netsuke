"""Hold Netsuke's Namespace runner-profile assignments.

The repository owns direct Linux, Windows, and ARM64 macOS job placement,
while Intel macOS jobs and externally defined reusable workflows retain their
existing runner ownership. These contracts prevent an unrelated workflow edit
from silently moving repository-owned work back to GitHub-hosted runners.

Run via ``make test-workflow-contracts``.
"""

import pytest
from namespace_runner_invariants import (
    CALLEE_SELECTED_RUNNER,
    REQUIRED_RUNNER_ASSIGNMENTS,
    has_required_runner_assignments,
    is_valid_ninja_sequence,
    is_valid_windows_tool_path_sequence,
)
from workflow_loading import (
    REPO_ROOT,
    job_steps,
    load_workflow,
    require_mapping,
    workflow_job,
)

WORKFLOW_DIR = REPO_ROOT / ".github" / "workflows"
DIRECT_RUNNER_SOURCES = (
    ("ci.build-test", "ci.yml", "build-test"),
    ("ci.build-test-windows", "ci.yml", "build-test-windows"),
    ("ci.windows-native-recipe-smoke", "ci.yml", "windows-native-recipe-smoke"),
    ("ci.kani-smoke", "ci.yml", "kani-smoke"),
    ("coverage-main.coverage-upload", "coverage-main.yml", "coverage-upload"),
    (
        "delayed-pr-comment.delay_and_comment",
        "delayed-pr-comment.yml",
        "delay_and_comment",
    ),
    ("netsukefile-test.netsukefile", "netsukefile-test.yml", "netsukefile"),
    ("release.metadata", "release.yml", "metadata"),
    (
        "release.windows-native-recipe-smoke",
        "release.yml",
        "windows-native-recipe-smoke",
    ),
    ("release.release", "release.yml", "release"),
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
        ("coverage-main.yml", "coverage-upload", "Test and Measure Coverage"),
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
    assert is_valid_ninja_sequence(steps, first_consumer), (
        f"{workflow_name} job {job_name} must install one pinned Ninja action "
        f"before {first_consumer!r}"
    )


def test_main_coverage_requires_real_ninja() -> None:
    """Reject a main baseline that silently skips real-Ninja tests."""
    workflow = load_workflow(WORKFLOW_DIR / "coverage-main.yml")
    coverage_job = workflow_job(workflow, "coverage-upload")
    env = require_mapping(coverage_job.get("env"), "jobs.coverage-upload.env")
    assert env.get("NETSUKE_REQUIRE_NINJA") == "1", (
        "coverage-main.yml must require real-Ninja integration coverage"
    )


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
    assert is_valid_windows_tool_path_sequence(steps), (
        "build-and-package.yml must expose the known-folder .dotnet tools "
        "directory exactly once before Windows packaging"
    )


def test_checked_in_workflows_satisfy_runner_assignment_contract() -> None:
    """Apply the generated runner-assignment model to checked-in workflows."""
    assignments = _checked_in_runner_assignments()
    assert has_required_runner_assignments(assignments), (
        "checked-in runner assignments must match the platform contract; "
        f"expected {REQUIRED_RUNNER_ASSIGNMENTS!r}, got {assignments!r}"
    )


def _checked_in_runner_assignments() -> dict[str, str]:
    """Normalize checked-in direct, matrix, and reusable runner ownership."""
    assignments = {
        key: str(
            workflow_job(load_workflow(WORKFLOW_DIR / workflow), job).get("runs-on")
        )
        for key, workflow, job in DIRECT_RUNNER_SOURCES
    }

    release = load_workflow(WORKFLOW_DIR / "release.yml")
    for job_name in ("build-linux", "build-windows"):
        job = workflow_job(release, job_name)
        inputs = require_mapping(job.get("with"), f"jobs.{job_name}.with")
        assignments[f"release.{job_name}"] = str(inputs.get("runner"))

    build_macos = workflow_job(release, "build-macos")
    strategy = require_mapping(build_macos.get("strategy"), "jobs.build-macos.strategy")
    matrix = require_mapping(strategy.get("matrix"), "jobs.build-macos.strategy.matrix")
    includes = matrix.get("include")
    assert isinstance(includes, list), "the build-macos matrix include must be a list"
    assignments.update({
        f"release.macos.{item['target']}": str(item["runner"])
        for item in includes
        if isinstance(item, dict)
    })

    package = load_workflow(WORKFLOW_DIR / "build-and-package.yml")
    assignments["build-and-package.build"] = str(
        workflow_job(package, "build").get("runs-on")
    )
    assignments["mutation-testing.mutation"] = _external_runner_ownership(
        "mutation-testing.yml", "mutation"
    )
    assignments["dependabot-automerge.automerge"] = _external_runner_ownership(
        "dependabot-automerge.yml", "automerge"
    )
    return assignments


def _external_runner_ownership(workflow_name: str, job_name: str) -> str:
    """Normalize an external reusable workflow's callee-owned runner."""
    job = workflow_job(load_workflow(WORKFLOW_DIR / workflow_name), job_name)
    uses = str(job.get("uses", ""))
    if uses.startswith("leynos/shared-actions/.github/workflows/") and not job.get(
        "runs-on"
    ):
        return CALLEE_SELECTED_RUNNER
    return str(job.get("runs-on"))
