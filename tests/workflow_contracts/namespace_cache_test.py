"""Hold Namespace cache ownership and bounded-concurrency contracts.

Namespace profiles supply the cache-volume substrate. A workflow must not add a
second GitHub cache writer beside that volume, because concurrent cache owners
make warm-run behaviour and storage cost impossible to reason about.

Run via ``make test-workflow-contracts``.
"""

from workflow_loading import (
    REPO_ROOT,
    job_steps,
    load_workflow,
    named_step,
    require_mapping,
)

NAMESPACE_CACHE_ACTION = (
    "namespacelabs/nscloud-cache-action@c5f8dab7560444c4bf8dbc64f1b203431873c547"
)
SETUP_RUST_ACTION = "leynos/shared-actions/.github/actions/setup-rust@"
EXTERNAL_CACHE_PROVIDER = "external"
# The Kani job redirects CARGO_HOME into the workspace, so its Cargo downloads
# land under the job-local Cargo home rather than the runner's `~/.cargo`.
CARGO_DOWNLOAD_PATHS = {("ci.yml", "kani-smoke"): ".kani-cargo"}
CACHE_SUMMARY_STEPS = {
    "ci.yml": {
        "build-test": "Summarize Namespace cache",
        "kani-smoke": "Summarize Kani cache",
    },
    "ci-windows.yml": {
        "build-test-windows": "Summarize Namespace cache",
        "windows-native-recipe-smoke": "Summarize Namespace cache",
    },
    "coverage-main.yml": {"coverage-upload": "Summarize Namespace cache"},
    "netsukefile-test.yml": {"netsukefile": "Summarize Namespace cache"},
    "release.yml": {"windows-native-recipe-smoke": "Summarize Namespace cache"},
}


#: Every job whose runner attaches a Namespace cache volume.
CACHE_VOLUME_JOBS = {
    "ci.yml": ("build-test", "kani-smoke"),
    "ci-windows.yml": ("build-test-windows", "windows-native-recipe-smoke"),
    "coverage-main.yml": ("coverage-upload",),
    "netsukefile-test.yml": ("netsukefile",),
    "release.yml": ("windows-native-recipe-smoke",),
}


def _sole_cache_step(steps: list[dict[str, object]], label: str) -> dict[str, object]:
    """Return the job's single Namespace cache step, pinned as expected."""
    cache_steps = [
        step for step in steps if "nscloud-cache-action" in str(step.get("uses", ""))
    ]
    assert len(cache_steps) == 1, (
        f"{label} must have one Namespace cache owner, got {cache_steps!r}"
    )
    assert cache_steps[0].get("uses") == NAMESPACE_CACHE_ACTION, (
        f"{label} must pin the Namespace cache action"
    )
    return cache_steps[0]


def _assert_cache_mounts_durable_paths(
    cache_step: dict[str, object], key: tuple[str, str], label: str
) -> None:
    """Require explicit Cargo download paths and no `rust` preset."""
    cache_inputs = require_mapping(cache_step.get("with"), "Namespace cache inputs")
    assert "rust" not in str(cache_inputs.get("cache", "")).splitlines(), (
        f"{label} must not mount target via rust mode"
    )
    expected = CARGO_DOWNLOAD_PATHS.get(key, "~/.cargo/registry")
    assert expected in str(cache_inputs.get("path", "")), (
        f"{label} must retain Cargo downloads under {expected}"
    )


def _assert_cache_precedes_installs(
    steps: list[dict[str, object]], cache_step: dict[str, object], label: str
) -> None:
    """Require the volume to be mounted after checkout and before any install."""
    checkout_index = next(
        index
        for index, step in enumerate(steps)
        if "actions/checkout@" in str(step.get("uses", ""))
    )
    cache_index = steps.index(cache_step)
    assert checkout_index < cache_index, (
        f"{label} must configure the cache after checkout"
    )
    first_install_index = next(
        (
            index
            for index, step in enumerate(steps)
            if index > checkout_index
            and (
                str(step.get("name", "")).startswith("Install ")
                or "setup-rust@" in str(step.get("uses", ""))
            )
        ),
        len(steps),
    )
    assert cache_index < first_install_index, (
        f"{label} must mount the cache before installs"
    )


def test_namespace_cache_volume_has_one_pinned_owner() -> None:
    """Require each Namespace cache user to pin the volume action exactly once."""
    workflow_dir = REPO_ROOT / ".github" / "workflows"
    for workflow_name, job_names in CACHE_VOLUME_JOBS.items():
        workflow = load_workflow(workflow_dir / workflow_name)
        for job_name in job_names:
            label = f"{workflow_name} {job_name}"
            steps = job_steps(workflow, job_name)
            cache_step = _sole_cache_step(steps, label)
            _assert_cache_mounts_durable_paths(
                cache_step, (workflow_name, job_name), label
            )
            _assert_cache_precedes_installs(steps, cache_step, label)


def test_every_namespace_cache_reports_its_effectiveness() -> None:
    """Require each mounted volume to publish its `cache-hit` unconditionally.

    A summary that runs only on success hides the cold-run case that the
    cache exists to eliminate, so the step must also carry `if: always()`.
    """
    workflow_dir = REPO_ROOT / ".github" / "workflows"
    for workflow_name, summaries in CACHE_SUMMARY_STEPS.items():
        workflow = load_workflow(workflow_dir / workflow_name)
        for job_name, summary_name in summaries.items():
            steps = job_steps(workflow, job_name)
            summary = named_step(steps, summary_name)
            assert summary.get("if") == "always()", (
                f"{workflow_name} {job_name} must summarize the cache on every run"
            )
            summary_command = str(summary.get("run", ""))
            assert "steps.namespace_cache.outputs.cache-hit" in summary_command, (
                f"{workflow_name} {job_name} must report the volume's cache-hit output"
            )
            cache_step = next(
                step
                for step in steps
                if "nscloud-cache-action" in str(step.get("uses", ""))
            )
            assert cache_step.get("id") == "namespace_cache", (
                f"{workflow_name} {job_name} must identify its cache step so the "
                "summary can read its output"
            )
            assert steps.index(cache_step) < steps.index(summary), (
                f"{workflow_name} {job_name} must mount the volume before summarizing"
            )


def test_setup_rust_delegates_cache_ownership_to_namespace() -> None:
    """Require setup-rust to leave cache ownership to the mounted volume."""
    workflow_dir = REPO_ROOT / ".github" / "workflows"
    for workflow_name, job_names in CACHE_VOLUME_JOBS.items():
        workflow = load_workflow(workflow_dir / workflow_name)
        for job_name in job_names:
            setup = named_step(job_steps(workflow, job_name), "Setup Rust")
            assert str(setup.get("uses", "")).startswith(SETUP_RUST_ACTION), (
                f"{workflow_name} {job_name} must use shared setup-rust"
            )
            inputs = require_mapping(setup.get("with"), "Setup Rust inputs")
            assert inputs.get("cache-provider") == EXTERNAL_CACHE_PROVIDER, (
                f"{workflow_name} {job_name} must disable setup-rust's GitHub cache"
            )
            assert inputs.get("use-sccache") == "false", (
                f"{workflow_name} {job_name} must not enable GitHub-backed sccache"
            )


def test_workflows_do_not_reintroduce_github_cache_or_source_tool_builds() -> None:
    """Reject duplicate cache writers and direct Cargo tool compilation in CI."""
    workflow_dir = REPO_ROOT / ".github" / "workflows"
    workflow_text = "\n".join(
        workflow_path.read_text(encoding="utf-8")
        for workflow_path in sorted(workflow_dir.glob("*.yml"))
    )
    assert "actions/cache@" not in workflow_text, (
        "Namespace cache volumes must be the only workflow cache owner"
    )
    assert "cargo install " not in workflow_text, (
        "CI tools must use trusted prebuilt binaries rather than source builds"
    )
    assert "cache: rust" not in workflow_text, (
        "the Namespace rust preset must not mount the disposable target directory"
    )


def test_ci_bounds_nextest_workers() -> None:
    """Keep the four-vCPU Linux merge gate bounded."""
    workflow = load_workflow()
    build_job = require_mapping(workflow.get("jobs"), "workflow jobs")["build-test"]
    env = require_mapping(
        require_mapping(build_job, "build-test").get("env"), "build-test env"
    )
    assert env.get("BUILD_JOBS") == "-j 4", "Make must use four worker processes"
    assert env.get("NEXTEST_BUILD_JOBS") == "--build-jobs 4", (
        "nextest builds must use four worker processes"
    )
    assert env.get("NEXTEST_TEST_JOBS") == "-j 4", (
        "nextest tests must use four worker processes"
    )
    makefile = (REPO_ROOT / "Makefile").read_text(encoding="utf-8")
    assert "NEXTEST_TEST_JOBS ?=" in makefile, (
        "Makefile must provide an overridable nextest worker count"
    )
    assert "$(NEXTEST_TEST_JOBS)" in makefile, (
        "the nextest command must consume the configured worker count"
    )


def _assert_sccache_contract(
    workflow_name: str, job_name: str, expected_cache_path: str
) -> None:
    """Require one observable, binary-installed sccache owner for a job."""
    workflow = load_workflow(REPO_ROOT / ".github" / "workflows" / workflow_name)
    job = require_mapping(
        require_mapping(workflow.get("jobs"), "workflow jobs")[job_name], job_name
    )
    env = require_mapping(job.get("env"), f"{job_name} env")
    assert env.get("RUSTC_WRAPPER") == "sccache", (
        f"{workflow_name} {job_name} must compile through sccache"
    )

    steps = job_steps(workflow, job_name)
    cache_inputs = require_mapping(
        named_step(steps, "Set up Namespace cache volume").get("with"),
        "Namespace cache inputs",
    )
    assert expected_cache_path in str(cache_inputs.get("path", "")), (
        f"{workflow_name} {job_name} must retain its sccache directory"
    )
    sccache_install = named_step(steps, "Install sccache")
    assert sccache_install.get("uses") == (
        "taiki-e/install-action@18b1216eba7f8039b0f8d131d5473787f0edce68"
    ), f"{workflow_name} {job_name} must use the pinned sccache binary installer"
    sccache_inputs = require_mapping(
        sccache_install.get("with"), "sccache installer inputs"
    )
    assert sccache_inputs.get("tool") == "sccache@0.16.0", (
        "sccache must use the exact tested release"
    )
    assert sccache_inputs.get("fallback") == "none", (
        "sccache installation must not fall back to a source build"
    )
    assert "sccache --zero-stats" in str(
        named_step(steps, "Reset sccache statistics").get("run")
    ), f"{workflow_name} {job_name} must reset sccache statistics before compiling"
    assert "sccache --show-stats --stats-format=json" in str(
        named_step(steps, "Show sccache statistics").get("run")
    ), f"{workflow_name} {job_name} must export machine-readable sccache statistics"


def test_compiling_namespace_jobs_report_sccache_json() -> None:
    """Require compiler-cache observability on direct Namespace Rust builds."""
    expected_jobs = {
        ("ci.yml", "build-test"): "~/.cache/sccache",
        ("ci-windows.yml", "build-test-windows"): ".sccache",
        ("ci-windows.yml", "windows-native-recipe-smoke"): ".sccache",
        ("netsukefile-test.yml", "netsukefile"): "~/.cache/sccache",
        ("release.yml", "windows-native-recipe-smoke"): ".sccache",
    }
    for (workflow_name, job_name), cache_path in expected_jobs.items():
        _assert_sccache_contract(workflow_name, job_name, cache_path)


def test_coverage_delegates_archive_cache_ownership_to_namespace() -> None:
    """Require coverage actions to disable overlapping GitHub archives."""
    workflow_dir = REPO_ROOT / ".github" / "workflows"
    expected_jobs = {
        "ci.yml": "build-test",
        "coverage-main.yml": "coverage-upload",
    }
    for workflow_name, job_name in expected_jobs.items():
        workflow = load_workflow(workflow_dir / workflow_name)
        coverage = named_step(
            job_steps(workflow, job_name), "Test and Measure Coverage"
        )
        inputs = require_mapping(coverage.get("with"), "coverage inputs")
        assert inputs.get("cache-provider") == EXTERNAL_CACHE_PROVIDER, (
            f"{workflow_name} {job_name} must disable coverage archive caches"
        )


#: Jobs that install the Whitaker Dylint suite, and the cache-volume path that
#: owns the installer's platform-specific data directory.
WHITAKER_JOBS = {
    ("ci.yml", "build-test"): "~/.local/share/whitaker",
    ("ci-windows.yml", "build-test-windows"): "~/AppData/Roaming/github/whitaker",
}


def test_whitaker_installs_against_a_caller_owned_cache() -> None:
    """Require the volume to own Whitaker's data and the clone guard to run.

    `whitaker-installer` 0.2.7 decides between cloning and pulling on
    directory existence alone, and mounting the volume creates that directory
    on a cold run. Without the guard the installer runs `git pull` against a
    directory holding no repository and the gate fails before it lints
    anything.
    """
    workflow_dir = REPO_ROOT / ".github" / "workflows"
    for (workflow_name, job_name), data_path in WHITAKER_JOBS.items():
        steps = job_steps(load_workflow(workflow_dir / workflow_name), job_name)
        cache_inputs = require_mapping(
            named_step(steps, "Set up Namespace cache volume").get("with"),
            "Namespace cache inputs",
        )
        assert data_path in str(cache_inputs.get("path", "")).splitlines(), (
            f"{workflow_name} {job_name} must cache Whitaker's data directory "
            f"at {data_path}"
        )
        install = named_step(steps, "Install Whitaker")
        install_inputs = require_mapping(install.get("with"), "Install Whitaker inputs")
        assert install_inputs.get("cache-provider") == EXTERNAL_CACHE_PROVIDER, (
            f"{workflow_name} {job_name} must let the volume own the Whitaker cache"
        )
        guard = named_step(steps, "Prepare Whitaker cache directory")
        assert steps.index(guard) < steps.index(install), (
            f"{workflow_name} {job_name} must clear a non-repository Whitaker "
            "data directory before running the installer"
        )
