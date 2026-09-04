"""Hold the compiler-cache contract for every job that compiles Rust.

sccache arrives as a checksum-verified prebuilt binary, exactly one backend is
active per job, and every compiling job resets its counters before building and
reports them afterwards even on failure. Zero compile requests is a failed
integration, not a quiet no-op, so the statistics are part of the contract.

Run via ``make test-workflow-contracts``.
"""

import pytest
from cache_contract_data import (
    ACTION_DIR,
    SCCACHE_CREDENTIAL_JOBS,
    SCCACHE_CREDENTIALS_ACTION,
    SCCACHE_EXEMPT_LANE,
    SCCACHE_LOCAL_DIR_JOBS,
    SCCACHE_WRAPPER_JOBS,
    WORKFLOW_DIR,
    cache_steps,
    declared_paths,
    lane_steps,
)
from workflow_loading import (
    job_steps,
    load_workflow,
    named_step,
    require_mapping,
    workflow_job,
)


def _assert_sccache_contract(workflow_name: str, job_name: str) -> None:
    """Require one observable, binary-installed sccache owner for a job."""
    workflow = load_workflow(WORKFLOW_DIR / workflow_name)
    job = workflow_job(workflow, job_name)
    env = require_mapping(job.get("env"), f"{job_name} env")
    assert env.get("RUSTC_WRAPPER") == "sccache", (
        f"{workflow_name} {job_name} must compile through sccache"
    )

    steps = job_steps(workflow, job_name)
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
    reset = named_step(steps, "Reset sccache statistics")
    assert "sccache --zero-stats" in str(reset.get("run")), (
        f"{workflow_name} {job_name} must reset sccache statistics before compiling"
    )
    show = named_step(steps, "Show sccache statistics")
    assert show.get("if") == "always()", (
        f"{workflow_name} {job_name} must report sccache statistics on failure too"
    )
    show_command = str(show.get("run"))
    assert "sccache --show-stats --stats-format=json" in show_command, (
        f"{workflow_name} {job_name} must export machine-readable sccache statistics"
    )
    assert steps.index(reset) < steps.index(show), (
        f"{workflow_name} {job_name} must reset its counters before reporting them"
    )


@pytest.mark.parametrize(
    ("workflow_name", "job_name"),
    [
        ("ci.yml", "build-test"),
        ("ci-windows.yml", "build-test-windows"),
        ("ci-windows.yml", "windows-native-recipe-smoke"),
        ("netsukefile-test.yml", "netsukefile"),
        ("coverage-main.yml", "coverage-upload"),
        ("release.yml", "windows-native-recipe-smoke"),
    ],
)
def test_compiling_jobs_report_sccache_statistics(
    workflow_name: str, job_name: str
) -> None:
    """Require compiler-cache observability on every direct Rust build."""
    _assert_sccache_contract(workflow_name, job_name)


def test_linux_gate_selects_exactly_one_sccache_backend() -> None:
    """Require the local-directory fallback to be wired but disabled.

    The GitHub Actions backend needs no archive of its own, so enabling both
    would give the compiler cache two owners. One repository variable selects
    between them.
    """
    workflow = load_workflow(WORKFLOW_DIR / "ci.yml")
    env = require_mapping(
        workflow_job(workflow, "build-test").get("env"), "build-test env"
    )
    assert env.get("SCCACHE_GHA_ENABLED") == (
        "${{ vars.NETSUKE_SCCACHE_LOCAL_DIR == 'true' && 'false' || 'true' }}"
    ), "the sccache backend must be selected by one repository variable"

    steps = lane_steps(ACTION_DIR / "linux-gate-cache" / "action.yml", None)
    local_steps = [
        step for step in steps if "sccache-local'] == 'true'" in str(step.get("if"))
    ]
    assert len(local_steps) == 2, (
        "the local-directory backend must be wired for restore and save only, "
        f"got {local_steps!r}"
    )


@pytest.mark.parametrize(("workflow_name", "job_name"), SCCACHE_CREDENTIAL_JOBS)
def test_credentials_are_exported_before_the_server_can_start(
    workflow_name: str, job_name: str
) -> None:
    """Require the credential export before anything starts sccache.

    A `run` step on Ubicloud cannot see `ACTIONS_RESULTS_URL` or
    `ACTIONS_RUNTIME_TOKEN`, and the shared Rust setup action that normally
    publishes them is disabled here. `sccache --zero-stats`, `--start-server`,
    and the first wrapped `rustc` each start the server, and a server started
    without those variables stays in local-disk mode for the whole job and
    reports zero compile requests.
    """
    steps = job_steps(load_workflow(WORKFLOW_DIR / workflow_name), job_name)
    export_indices = [
        index
        for index, step in enumerate(steps)
        if str(step.get("uses", "")) == SCCACHE_CREDENTIALS_ACTION
    ]
    assert len(export_indices) == 1, (
        f"{workflow_name} {job_name} must export sccache credentials exactly "
        f"once, got {export_indices!r}"
    )
    export_index = export_indices[0]
    checkout_indices = [
        index
        for index, step in enumerate(steps)
        if "actions/checkout@" in str(step.get("uses", ""))
    ]
    assert checkout_indices, f"{workflow_name} {job_name} must check out first"
    assert export_index == checkout_indices[0] + 1, (
        f"{workflow_name} {job_name} must export credentials immediately after checkout"
    )
    starters = [
        index
        for index, step in enumerate(steps)
        if str(step.get("name", "")) in {"Install sccache", "Reset sccache statistics"}
        or "sccache" in str(step.get("run", ""))
    ]
    assert starters, f"{workflow_name} {job_name} should touch sccache somewhere"
    assert export_index < min(starters), (
        f"{workflow_name} {job_name} must export credentials before anything "
        "can start the sccache server"
    )


@pytest.mark.parametrize(("workflow_name", "job_name"), SCCACHE_WRAPPER_JOBS)
def test_every_compiling_job_reaches_the_compiler_cache(
    workflow_name: str, job_name: str
) -> None:
    """Require every Rust build to compile through sccache.

    The coverage and Netsukefile builds produce different object shapes from
    the merge gate, but sccache hashes the flags, so all of them share one
    store and none needs a `target` archive. A job that omits the wrapper
    silently rebuilds everything and reports zero compile requests.
    """
    job = workflow_job(load_workflow(WORKFLOW_DIR / workflow_name), job_name)
    env = require_mapping(job.get("env"), f"{job_name} env")
    assert env.get("RUSTC_WRAPPER") == "sccache", (
        f"{workflow_name} {job_name} must compile through sccache"
    )
    assert env.get("SCCACHE_CACHE_SIZE") == "4G", (
        f"{workflow_name} {job_name} must size its store for two build "
        f"shapes, got {env.get('SCCACHE_CACHE_SIZE')!r}"
    )


def test_the_windows_packaging_lane_compiles_without_a_wrapper() -> None:
    """Require the packaging lane to leave Windows uncached, and only Windows.

    sccache re-spawns rustc there with the target's whole `--extern` and `-L`
    list and exceeds the operating system's command-line limit. Release builds
    are infrequent, so the lane runs uncached rather than unreliably; the
    Windows merge gate keeps its own local sccache.
    """
    workflow_name, job_name = SCCACHE_EXEMPT_LANE
    job = workflow_job(load_workflow(WORKFLOW_DIR / workflow_name), job_name)
    env = require_mapping(job.get("env"), f"{job_name} env")
    windows_off = "${{ inputs.platform == 'windows' && '' || 'sccache' }}"
    assert env.get("RUSTC_WRAPPER") == windows_off, (
        f"{workflow_name} must clear the wrapper on Windows only, got "
        f"{env.get('RUSTC_WRAPPER')!r}"
    )
    steps = job_steps(load_workflow(WORKFLOW_DIR / workflow_name), job_name)
    installer = named_step(steps, "Install sccache")
    assert installer.get("if") == "inputs.platform != 'windows'", (
        "the packaging lane must skip the installer on the uncached platform"
    )
    build = named_step(steps, "Build release binary")
    inputs = require_mapping(build.get("with"), "build inputs")
    assert inputs.get("use-sccache") == (
        "${{ inputs.platform == 'windows' && 'false' || 'true' }}"
    ), "the nested action must not enable sccache on the uncached platform"
    assert not [
        step for step in cache_steps(steps) if ".sccache" in str(declared_paths(step))
    ], "the uncached lane must keep no compiler-cache archive"


@pytest.mark.parametrize(("workflow_name", "job_name"), SCCACHE_LOCAL_DIR_JOBS)
def test_windows_lanes_use_a_workspace_compiler_cache(
    workflow_name: str, job_name: str
) -> None:
    """Require the Windows lanes to keep sccache in a cached workspace directory.

    The GitHub Actions backend rate-limited every write on this platform, so
    the compiler cache travels as an archive instead. Setting the backend flag
    here would give the compiler cache two owners again.
    """
    job = workflow_job(load_workflow(WORKFLOW_DIR / workflow_name), job_name)
    env = require_mapping(job.get("env"), f"{job_name} env")
    assert env.get("SCCACHE_DIR") == "${{ github.workspace }}/.sccache", (
        f"{workflow_name} {job_name} must name a workspace compiler cache, "
        f"got {env.get('SCCACHE_DIR')!r}"
    )
    assert "SCCACHE_GHA_ENABLED" not in env, (
        f"{workflow_name} {job_name} must not re-enable the rate-limited backend"
    )


def test_the_export_names_the_proxy_endpoint_not_the_results_service() -> None:
    """Require the export to point sccache at the endpoint Ubicloud serves.

    Ubicloud intercepts the cache service with a local proxy advertised as
    `ACTIONS_CACHE_URL`, which serves the v1 API. sccache 0.16 prefers
    GitHub's v2 results service whenever `ACTIONS_CACHE_SERVICE_V2` is set,
    and that address resolves past the proxy to GitHub. Exporting
    `ACTIONS_RESULTS_URL` did exactly that: 5310 requests, zero hits, and one
    write error per miss, with every object landing in GitHub's store.
    """
    steps = lane_steps(ACTION_DIR / "sccache-gha-credentials" / "action.yml", None)
    script = str(
        require_mapping(steps[0].get("with"), "export inputs").get("script", "")
    )
    required = {
        "ACTIONS_CACHE_URL": "publish the proxy address sccache should use",
        "ACTIONS_RUNTIME_TOKEN": "publish the token that address requires",
        "ACTIONS_CACHE_SERVICE_V2', ''": (
            "clear the v2 switch, which routes past the proxy"
        ),
    }
    missing = [reason for token, reason in required.items() if token not in script]
    assert not missing, f"the export must {'; '.join(missing)}"
    assert "ACTIONS_RESULTS_URL" not in script, (
        "the export must not publish the v2 results service address, which is "
        "what sent 92 sccache objects to GitHub instead of Ubicloud"
    )


def test_orthohelp_probes_before_either_installer() -> None:
    """Require the version probe to precede both installation paths.

    The cache restores `~/.cargo/bin`, and `cargo install` refuses to
    overwrite a binary already there, so a warm run that reached either
    installer failed on a cache hit.
    """
    steps = job_steps(load_workflow(WORKFLOW_DIR / "build-and-package.yml"), "build")
    script = str(named_step(steps, "Install cargo-orthohelp").get("run", ""))
    probe = script.index("cargo-orthohelp --version")
    assert probe < script.index("cargo binstall"), (
        "the probe must precede the binary installer"
    )
    assert probe < script.index("cargo install --locked"), (
        "the probe must precede the source fallback"
    )
    assert "exit 0" in script[probe : script.index("cargo binstall")], (
        "a matching probe must skip both installers rather than fall through"
    )
