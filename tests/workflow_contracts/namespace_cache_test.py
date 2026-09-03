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


def test_namespace_cache_volume_has_one_pinned_owner() -> None:
    """Require each Linux Namespace cache user to pin the volume action once."""
    expected_jobs = {
        "ci.yml": (
            "build-test",
            "build-test-windows",
            "windows-native-recipe-smoke",
            "kani-smoke",
        ),
        "coverage-main.yml": ("coverage-upload",),
        "netsukefile-test.yml": ("netsukefile",),
        "release.yml": ("windows-native-recipe-smoke",),
    }
    workflow_dir = REPO_ROOT / ".github" / "workflows"
    for workflow_name, job_names in expected_jobs.items():
        workflow = load_workflow(workflow_dir / workflow_name)
        for job_name in job_names:
            steps = job_steps(workflow, job_name)
            cache_steps = [
                step
                for step in steps
                if "nscloud-cache-action" in str(step.get("uses", ""))
            ]
            assert len(cache_steps) == 1, (
                f"{workflow_name} {job_name} must have one Namespace cache owner, "
                f"got {cache_steps!r}"
            )
            assert cache_steps[0].get("uses") == NAMESPACE_CACHE_ACTION, (
                f"{workflow_name} {job_name} must pin the Namespace cache action"
            )
            cache_inputs = require_mapping(
                cache_steps[0].get("with"), "Namespace cache inputs"
            )
            assert "rust" not in str(cache_inputs.get("cache", "")).splitlines(), (
                f"{workflow_name} {job_name} must not mount target via rust mode"
            )
            cached_paths = str(cache_inputs.get("path", ""))
            assert "~/.cargo/registry" in cached_paths, (
                f"{workflow_name} {job_name} must retain Cargo downloads"
            )
            cache_index = steps.index(cache_steps[0])
            checkout_index = next(
                index
                for index, step in enumerate(steps)
                if "actions/checkout@" in str(step.get("uses", ""))
            )
            assert checkout_index < cache_index, (
                f"{workflow_name} {job_name} must configure the cache after checkout"
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
                f"{workflow_name} {job_name} must mount the cache before installs"
            )


def test_setup_rust_delegates_cache_ownership_to_namespace() -> None:
    """Require setup-rust to leave cache ownership to the mounted volume."""
    workflow_dir = REPO_ROOT / ".github" / "workflows"
    expected_jobs = {
        "ci.yml": (
            "build-test",
            "build-test-windows",
            "windows-native-recipe-smoke",
            "kani-smoke",
        ),
        "coverage-main.yml": ("coverage-upload",),
        "netsukefile-test.yml": ("netsukefile",),
        "release.yml": ("windows-native-recipe-smoke",),
    }
    for workflow_name, job_names in expected_jobs.items():
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


def test_kani_uses_cached_prebuilt_frontend_and_release_bundle() -> None:
    """Require Kani's separate front-end and verifier payloads to be cacheable."""
    workflow = load_workflow()
    job = require_mapping(
        require_mapping(workflow.get("jobs"), "workflow jobs")["kani-smoke"],
        "kani-smoke",
    )
    env = require_mapping(job.get("env"), "kani-smoke env")
    assert env.get("CARGO_HOME") == "${{ github.workspace }}/.kani-cargo", (
        "Kani's Cargo front-end must live in the cached job-local Cargo home"
    )
    assert env.get("KANI_HOME") == "${{ github.workspace }}/.kani-home", (
        "Kani's verifier payload must live in the cached job-local Kani home"
    )
    assert env.get("RUSTUP_HOME") == "${{ github.workspace }}/.kani-rustup", (
        "Kani's supporting toolchain must live in the cached job-local Rustup home"
    )

    steps = job_steps(workflow, "kani-smoke")
    cache_inputs = require_mapping(
        named_step(steps, "Set up Kani cache volume").get("with"),
        "Kani cache inputs",
    )
    cached_paths = str(cache_inputs.get("path", ""))
    for required_path in (".kani-cargo", ".kani-home", ".kani-rustup"):
        assert required_path in cached_paths, f"Kani must cache {required_path}"

    setup_inputs = require_mapping(
        named_step(steps, "Setup Rust").get("with"), "Setup Rust inputs"
    )
    assert setup_inputs.get("install-binstall") == "false", (
        "Kani installs its cached front-end directly from a pinned binary archive"
    )

    install_command = str(named_step(steps, "Install prebuilt Kani").get("run"))
    required_install_fragments = (
        "cargo-quickinstall/releases/download/kani-verifier-",
        "ed2bafc239b834e14c6b66fc4838e342",
        "model-checking/kani/releases/download/kani-",
        "3b5f7afd3b51603ee720db7bc1bc4fe4",
        '[[ ! -x "${cargo_bin}/cargo-kani"',
        '[[ ! -x "${kani_dir}/bin/kani-driver"',
        'cargo kani setup --use-local-bundle "${bundle}"',
    )
    missing_fragments = tuple(
        fragment
        for fragment in required_install_fragments
        if fragment not in install_command
    )
    assert not missing_fragments, (
        f"Kani's binary-only cached installation is missing {missing_fragments!r}"
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
        ("ci.yml", "build-test-windows"): ".sccache",
        ("ci.yml", "windows-native-recipe-smoke"): ".sccache",
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
