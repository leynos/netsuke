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
from actions_expressions import static_label
from fork_fallback import owned_runner
from runner_placement_invariants import (
    CALLEE_SELECTED_RUNNER,
    GITHUB_HOSTED_ONLY_KEYS,
    REQUIRED_RUNNER_ASSIGNMENTS,
    UBICLOUD_COMPAT_LABEL,
    UBICLOUD_DEFAULT_LABEL,
    UBICLOUD_LARGE_LABEL,
    has_required_runner_assignments,
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
    ("ci-windows.lint-windows", "ci-windows.yml", "lint-windows"),
    ("ci-windows.build-test-windows", "ci-windows.yml", "build-test-windows"),
    # The MSI gate selects its runner from a matrix and is expanded by
    # `_msi_matrix_legs` rather than read as one literal label.
    ("ci-macos.read-policy", "ci-macos.yml", "read-policy"),
    ("ci.kani-smoke", "ci.yml", "kani-smoke"),
    (
        "ci-default-features.default-features",
        "ci-default-features.yml",
        "default-features",
    ),
    ("coverage-main.coverage-upload", "coverage-main.yml", "coverage-upload"),
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
@pytest.mark.parametrize(
    ("workflow_name", "job_name", "expected_runner"),
    [
        ("ci.yml", "build-test", UBICLOUD_LARGE_LABEL),
        ("ci-windows.yml", "lint-windows", "windows-latest"),
        ("ci-windows.yml", "build-test-windows", "windows-latest"),
        ("ci.yml", "kani-smoke", UBICLOUD_DEFAULT_LABEL),
        ("coverage-main.yml", "coverage-upload", UBICLOUD_LARGE_LABEL),
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
    # A lane that serves pull requests names two runners. The table pins the
    # one this repository's own branches get; the fork arm is a GitHub-hosted
    # fallback whose shape this rule deliberately does not govern, and
    # `test_pull_request_lanes_fall_back_for_forks` pins it instead.
    actual_runner = owned_runner(workflow_job(workflow, job_name).get("runs-on"))
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
    placement at all; the rest are API-bound or not developer-blocking.
    """
    runner = REQUIRED_RUNNER_ASSIGNMENTS[assignment_key]
    assert not is_ubicloud_label(runner), (
        f"{assignment_key} must stay on a GitHub-hosted runner, got {runner!r}"
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


def test_windows_msi_gate_runs_one_leg_per_shipping_architecture() -> None:
    """Run the MSI gate on x64 and Arm64, each on a runner of that platform.

    The install location follows the package architecture, so an x64-only gate
    would leave the Arm64 program files folder unproven. Two explicitly named
    legs also stop a later edit from quietly dropping one architecture.
    """
    job = workflow_job(
        load_workflow(WORKFLOW_DIR / "ci-windows.yml"), "windows-msi-upgrade"
    )
    assert job.get("runs-on") == "${{ matrix.runner }}", (
        "the MSI gate must let its matrix legs decide the runner, "
        f"got {job.get('runs-on')!r}"
    )
    legs = _msi_matrix_legs()
    assert legs == {
        "x64": "windows-latest",
        "arm64": "windows-11-arm",
    }, f"unexpected Windows MSI gate matrix: {legs!r}"


@pytest.mark.parametrize(
    ("value", "label"),
    [
        ("${{ 'private-runner' }}", "private-runner"),
        ("${{ matrix.runner }}", None),
        ("${{ vars.PREFIX }}-private-${{ vars.SUFFIX }}", None),
    ],
)
def test_a_literal_expression_resolves_to_the_label_it_selects(
    value: str, label: str | None
) -> None:
    """A fixed label inside an expression must reach the label inventory.

    GitHub interpolates expressions inside strings, so a label expressed this
    way selects that runner exactly as the bare literal; unresolved, it would
    escape the registry comparison. A matrix selector is resolved against its
    job's own matrix in `runner_matrix`, which `runner_matrix_test` covers.
    """
    assert static_label(value) == label, (
        f"{value!r} must resolve to {label!r}, got {static_label(value)!r}"
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
        path.read_text(encoding="utf-8")
        for path in sorted({*WORKFLOW_DIR.glob("*.yml"), *WORKFLOW_DIR.glob("*.yaml")})
    )


def _msi_matrix_legs() -> dict[str, str]:
    """Return the Windows MSI gate's architecture-to-runner matrix legs.

    Returns
    -------
    dict[str, str]
        Each MSI architecture mapped to the runner label its leg uses.
    """
    job = workflow_job(
        load_workflow(WORKFLOW_DIR / "ci-windows.yml"), "windows-msi-upgrade"
    )
    strategy = require_mapping(job.get("strategy"), "jobs.windows-msi-upgrade.strategy")
    matrix = require_mapping(
        strategy.get("matrix"), "jobs.windows-msi-upgrade.strategy.matrix"
    )
    # An extra axis multiplies the legs GitHub creates without adding a matrix
    # entry, so only the include list is allowed here.
    assert set(matrix) == {"include"}, (
        "the windows-msi-upgrade matrix must declare only its include entries, "
        f"got {sorted(matrix)!r}"
    )
    includes = require_list(
        matrix.get("include"), "the windows-msi-upgrade matrix include"
    )
    # A duplicate architecture would collapse in the dictionary below and hide
    # its leg from every caller, so the entry count is checked first.
    assert len(includes) == 2, (
        "the windows-msi-upgrade matrix must declare exactly two legs, "
        f"got {includes!r}"
    )
    return {
        str(item["architecture"]): str(item["runner"])
        for item in includes
        if isinstance(item, dict)
    }


def _checked_in_runner_assignments() -> dict[str, str]:
    """Normalize checked-in direct, matrix, and reusable runner ownership."""
    assignments = {
        key: owned_runner(
            workflow_job(load_workflow(WORKFLOW_DIR / workflow), job).get("runs-on")
        )
        for key, workflow, job in DIRECT_RUNNER_SOURCES
    }
    # The MSI gate names one matrix leg per architecture, so its ownership is
    # read leg by leg rather than from the `${{ matrix.runner }}` expression.
    assignments.update({
        f"ci-windows.windows-msi-upgrade.{architecture}": runner
        for architecture, runner in _msi_matrix_legs().items()
    })

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
    assignments["delayed-pr-comment.delayed-comment"] = _external_runner_ownership(
        "delayed-pr-comment.yml", "delayed-comment"
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
