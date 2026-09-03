"""Hold Netsuke's Ubicloud and GitHub-hosted runner placement.

Ubicloud offers Linux runners only, so the repository splits its estate: the
Linux jobs that block a developer run on `ubicloud-standard-2-ubuntu-2404`
(and the `-2204` variant for the deliberate compatibility lane), while Windows,
macOS, scheduled, and administrative jobs stay on GitHub-hosted runners. These
contracts prevent an unrelated workflow edit from silently changing that
ownership, oversubscribing a two-vCPU shape, or leaving a Ubicloud job without
a timeout.

Run via ``make test-workflow-contracts``.
"""

import pytest
import yaml
from runner_placement_invariants import (
    CALLEE_SELECTED_RUNNER,
    GITHUB_HOSTED_ONLY_KEYS,
    LANE_VCPUS,
    REQUIRED_RUNNER_ASSIGNMENTS,
    UBICLOUD_COMPAT_LABEL,
    UBICLOUD_DEFAULT_LABEL,
    UBICLOUD_LABELS,
    UBICLOUD_LARGE_LABEL,
    has_required_runner_assignments,
    is_bounded_worker_count,
    is_ubicloud_label,
    is_valid_ninja_sequence,
    is_valid_windows_tool_path_sequence,
)
from workflow_loading import (
    REPO_ROOT,
    job_steps,
    load_workflow,
    require_list,
    require_mapping,
    workflow_job,
)

WORKFLOW_DIR = REPO_ROOT / ".github" / "workflows"
DIRECT_RUNNER_SOURCES = (
    ("ci.build-test", "ci.yml", "build-test"),
    ("ci-windows.build-test-windows", "ci-windows.yml", "build-test-windows"),
    (
        "ci-windows.windows-native-recipe-smoke",
        "ci-windows.yml",
        "windows-native-recipe-smoke",
    ),
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

#: Jobs placed directly on a Ubicloud label, with the worker-count variables
#: each one must keep within the shape's vCPU count.
UBICLOUD_WORKER_BOUNDS = (
    # The instrumented run reads cargo's and nextest's own variables rather
    # than the Make variables the folded-away test step consumed.
    (
        "ci.yml",
        "build-test",
        ("BUILD_JOBS", "CARGO_BUILD_JOBS", "NEXTEST_TEST_THREADS"),
    ),
    ("netsukefile-test.yml", "netsukefile", ("BUILD_JOBS",)),
)


@pytest.mark.parametrize(
    ("workflow_name", "job_name", "expected_runner"),
    [
        ("ci.yml", "build-test", UBICLOUD_LARGE_LABEL),
        ("ci-windows.yml", "build-test-windows", "windows-latest"),
        ("ci-windows.yml", "windows-native-recipe-smoke", "windows-latest"),
        ("ci.yml", "kani-smoke", UBICLOUD_DEFAULT_LABEL),
        ("coverage-main.yml", "coverage-upload", UBICLOUD_DEFAULT_LABEL),
        ("delayed-pr-comment.yml", "delay_and_comment", "ubuntu-latest"),
        ("netsukefile-test.yml", "netsukefile", UBICLOUD_COMPAT_LABEL),
        ("release.yml", "metadata", "ubuntu-latest"),
        ("release.yml", "windows-native-recipe-smoke", "windows-latest"),
        ("release.yml", "release", "ubuntu-latest"),
    ],
)
def test_repository_owned_jobs_use_required_runners(
    workflow_name: str,
    job_name: str,
    expected_runner: str,
) -> None:
    """Require each repository-owned job to use its intended runner."""
    workflow = load_workflow(WORKFLOW_DIR / workflow_name)
    actual_runner = workflow_job(workflow, job_name).get("runs-on")
    assert actual_runner == expected_runner, (
        f"{workflow_name} job {job_name} must run on {expected_runner}, "
        f"got {actual_runner!r}"
    )


@pytest.mark.parametrize("assignment_key", GITHUB_HOSTED_ONLY_KEYS)
def test_windows_macos_and_administrative_jobs_stay_github_hosted(
    assignment_key: str,
) -> None:
    """Keep every job without a Ubicloud option on a GitHub-hosted runner.

    Ubicloud publishes Ubuntu images only, so a Windows or macOS job has no
    Ubicloud placement at all. The remaining jobs here are API-bound or not
    developer-blocking, so a build shape would buy nothing.
    """
    runner = REQUIRED_RUNNER_ASSIGNMENTS[assignment_key]
    assert not is_ubicloud_label(runner), (
        f"{assignment_key} must stay on a GitHub-hosted runner, got {runner!r}"
    )


@pytest.mark.parametrize(
    ("workflow_name", "job_name"),
    [
        ("ci.yml", "build-test"),
        ("ci.yml", "kani-smoke"),
        ("coverage-main.yml", "coverage-upload"),
        ("netsukefile-test.yml", "netsukefile"),
    ],
)
def test_every_ubicloud_job_declares_a_timeout(
    workflow_name: str, job_name: str
) -> None:
    """Require a timeout so a stuck Ubicloud VM cannot bill indefinitely."""
    job = workflow_job(load_workflow(WORKFLOW_DIR / workflow_name), job_name)
    timeout = job.get("timeout-minutes")
    assert isinstance(timeout, int), (
        f"{workflow_name} job {job_name} must set timeout-minutes, got {timeout!r}"
    )
    assert timeout > 0, (
        f"{workflow_name} job {job_name} must set a positive timeout, got {timeout!r}"
    )


@pytest.mark.parametrize(
    ("workflow_name", "job_name", "flag_names"), UBICLOUD_WORKER_BOUNDS
)
def test_worker_counts_match_the_lane_vcpu_count(
    workflow_name: str, job_name: str, flag_names: tuple[str, ...]
) -> None:
    """Keep compilation and test workers within the placed shape's vCPUs."""
    workflow = load_workflow(WORKFLOW_DIR / workflow_name)
    job = workflow_job(workflow, job_name)
    runner = str(job.get("runs-on"))
    vcpus = LANE_VCPUS[runner]
    env = require_mapping(job.get("env"), f"jobs.{job_name}.env")
    flags = {name: str(env[name]) for name in flag_names}
    assert is_bounded_worker_count(vcpus, flags), (
        f"{workflow_name} job {job_name} runs on {runner} with {vcpus} vCPUs "
        f"but declares {flags!r}"
    )
    declared = env.get("LINUX_LANE_VCPUS") or _workflow_env(workflow).get(
        "LINUX_LANE_VCPUS"
    )
    assert str(declared) == str(vcpus), (
        f"{workflow_name} job {job_name} must name its vCPU count once, "
        f"got {declared!r}"
    )


def test_windows_lane_names_its_vcpu_count_once() -> None:
    """Derive the Windows worker counts from one named constant."""
    workflow = load_workflow(WORKFLOW_DIR / "ci-windows.yml")
    vcpus = LANE_VCPUS["windows-latest"]
    assert str(_workflow_env(workflow).get("WINDOWS_LANE_VCPUS")) == str(vcpus), (
        "ci-windows.yml must declare the windows-latest vCPU count once"
    )
    job = workflow_job(workflow, "build-test-windows")
    env = require_mapping(job.get("env"), "jobs.build-test-windows.env")
    flags = {
        name: str(env[name])
        for name in ("BUILD_JOBS", "NEXTEST_BUILD_JOBS", "NEXTEST_TEST_JOBS")
    }
    assert is_bounded_worker_count(vcpus, flags), (
        f"build-test-windows declares {flags!r} for a {vcpus} vCPU runner"
    )


def test_actionlint_registers_exactly_the_ubicloud_labels_in_use() -> None:
    """Register every intentional Ubicloud label, and nothing else.

    actionlint rejects an unregistered self-hosted label, so a typo or an
    unreviewed shape fails the lint gate instead of queueing forever.
    """
    config = yaml.safe_load(
        (REPO_ROOT / ".github" / "actionlint.yaml").read_text(encoding="utf-8")
    )
    registered = require_mapping(config, "actionlint config")["self-hosted-runner"]
    labels = tuple(
        str(label)
        for label in require_list(
            require_mapping(registered, "self-hosted-runner").get("labels"),
            "self-hosted-runner labels",
        )
    )
    assert sorted(labels) == sorted(UBICLOUD_LABELS), (
        f"actionlint must register exactly {UBICLOUD_LABELS!r}, got {labels!r}"
    )
    workflow_text = _all_workflow_text()
    for label in labels:
        assert label in workflow_text, f"{label} is registered but never used"


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
def test_linux_jobs_install_ninja_before_use(
    workflow_name: str,
    job_name: str,
    first_consumer: str,
) -> None:
    """Provision pinned Ninja before a Ubicloud Linux job invokes it."""
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
        ("build-linux", UBICLOUD_DEFAULT_LABEL),
        ("build-windows", "windows-latest"),
    ],
)
def test_release_build_passes_its_platform_runner(
    job_name: str,
    expected_runner: str,
) -> None:
    """Require packaging to pass its platform's intended runner label."""
    workflow = load_workflow(WORKFLOW_DIR / "release.yml")
    build_job = workflow_job(workflow, job_name)
    inputs = require_mapping(build_job.get("with"), f"jobs.{job_name}.with")
    actual_runner = inputs.get("runner")
    assert actual_runner == expected_runner, (
        f"release.yml {job_name} must pass {expected_runner} to the package "
        f"workflow, got {actual_runner!r}"
    )


def test_macos_release_matrix_preserves_native_architectures() -> None:
    """Keep both macOS architectures on their GitHub-hosted runners."""
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
        "aarch64-apple-darwin": "macos-15",
    }, f"unexpected macOS runner matrix: {runners_by_target!r}"


def test_package_workflow_runs_on_its_caller_selected_runner() -> None:
    """Keep runner selection at the release caller's platform boundary."""
    workflow = load_workflow(WORKFLOW_DIR / "build-and-package.yml")
    actual_runner = workflow_job(workflow, "build").get("runs-on")
    assert actual_runner == "${{ inputs.runner }}", (
        "build-and-package.yml must use its caller-selected runner input, "
        f"got {actual_runner!r}"
    )


def test_package_workflow_limits_legacy_node_to_windows() -> None:
    """Permit the nested Node 20 action only for Windows packaging."""
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


def _workflow_env(workflow: dict[str, object]) -> dict[str, object]:
    """Return a workflow's top-level environment mapping."""
    return require_mapping(workflow.get("env", {}), "the workflow env")


def _all_workflow_text() -> str:
    """Return every workflow file's text, concatenated."""
    return "\n".join(
        path.read_text(encoding="utf-8") for path in sorted(WORKFLOW_DIR.glob("*.yml"))
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
