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
    SETUP_RUST_ACTION,
    WORKFLOW_DIR,
    cache_steps,
    declared_paths,
    lane_steps,
)
from sccache_compile_step_data import is_compile_step
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


@pytest.mark.parametrize(("workflow_name", "job_name"), SCCACHE_WRAPPER_JOBS)
def test_statistics_bracket_every_compile_step(
    workflow_name: str, job_name: str
) -> None:
    """Require the reset and report steps to bracket every compile step.

    `test_compiling_jobs_report_sccache_statistics` only orders the two
    statistic steps relative to each other; it never checks either one
    against the compile step that sits between them. A reset moved after
    the build, or a report moved before it, would still pass that ordering
    check while reporting zero or partial compile-request counts, which is
    the exact regression this test exists to catch.
    """
    steps = job_steps(load_workflow(WORKFLOW_DIR / workflow_name), job_name)
    reset_index = steps.index(named_step(steps, "Reset sccache statistics"))
    show_index = steps.index(named_step(steps, "Show sccache statistics"))
    compile_indices = [
        index for index, step in enumerate(steps) if is_compile_step(step)
    ]
    assert compile_indices, (
        f"{workflow_name} {job_name} must compile at least one step between "
        "resetting and reporting sccache statistics"
    )
    assert reset_index < min(compile_indices), (
        f"{workflow_name} {job_name} must reset sccache statistics before "
        "the first compile step"
    )
    assert max(compile_indices) < show_index, (
        f"{workflow_name} {job_name} must report sccache statistics after "
        "the last compile step"
    )


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


def test_the_packaging_lane_compiles_without_a_wrapper() -> None:
    """Require the release packaging lane to run with no compiler cache at all.

    Two independent reasons, each sufficient on its own. On Windows sccache
    re-spawns rustc with the aarch64 target's whole `--extern` and `-L` list
    and exceeds the operating system's command-line limit, which nothing here
    can shorten. Elsewhere the lane's server would be started inside the nested
    setup action, whose `mozilla-actions/sccache-action` re-exports
    `ACTIONS_CACHE_SERVICE_V2` and GitHub's own results address as its last
    act; on Ubicloud that sends every write past the cache proxy to GitHub,
    where it is rate-limited and lands in no store this repository reads.

    An earlier shape exempted Windows alone through a negated expression, and
    got the negation backwards once, which cost a release build. Requiring the
    variables to be absent outright removes the expression, and with it that
    whole class of mistake. Cargo treats an unset wrapper as no wrapper.
    """
    workflow_name, job_name = SCCACHE_EXEMPT_LANE
    job = workflow_job(load_workflow(WORKFLOW_DIR / workflow_name), job_name)
    env = require_mapping(job.get("env"), f"{job_name} env")
    for variable in ("RUSTC_WRAPPER", "SCCACHE_GHA_ENABLED", "SCCACHE_DIR"):
        assert variable not in env, (
            f"{workflow_name} {job_name} runs uncached and must not declare "
            f"{variable}, got {env.get(variable)!r}"
        )
    steps = job_steps(load_workflow(WORKFLOW_DIR / workflow_name), job_name)
    assert not [
        step for step in steps if str(step.get("name", "")) == "Install sccache"
    ], "the uncached lane must not install a compiler cache it never uses"
    build = named_step(steps, "Build release binary")
    inputs = require_mapping(build.get("with"), "build inputs")
    assert inputs.get("use-sccache") == "false", (
        "the nested action must not start a compiler cache on this lane, got "
        f"{inputs.get('use-sccache')!r}"
    )
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


def test_orthohelp_probes_before_installing() -> None:
    """Require the version probe to precede the installer.

    The cache restores `~/.cargo/bin`, and an install refuses to overwrite a
    binary already there, so a warm run that reached the installer failed on a
    cache hit.

    There is one installer to precede now rather than two. The source-build
    fallback this test also guarded was retired once `cargo-orthohelp` 0.9.1
    began publishing prebuilt archives (leynos/ortho-config#480).
    """
    steps = job_steps(load_workflow(WORKFLOW_DIR / "build-and-package.yml"), "build")
    script = str(named_step(steps, "Install cargo-orthohelp").get("run", ""))
    probe = script.index("cargo-orthohelp --version")
    installer = script.index("cargo binstall")
    assert probe < installer, "the probe must precede the installer"
    assert "exit 0" in script[probe:installer], (
        "a matching probe must skip the installer rather than fall through"
    )
    assert "cargo install" not in script, (
        "the retired source-build fallback must not return"
    )


@pytest.mark.parametrize(("workflow_name", "job_name"), SCCACHE_CREDENTIAL_JOBS)
def test_the_backend_flag_accompanies_the_wrapper(
    workflow_name: str, job_name: str
) -> None:
    """Require the backend flag beside the wrapper on every Ubicloud lane.

    `setup-rust` sets neither, so a job that names the wrapper without the
    flag gets a compiler cache on local disk that no archive retains and no
    later run reads.
    """
    job = workflow_job(load_workflow(WORKFLOW_DIR / workflow_name), job_name)
    env = require_mapping(job.get("env"), f"{job_name} env")
    assert env.get("RUSTC_WRAPPER") == "sccache", (
        f"{workflow_name} {job_name} must compile through sccache"
    )
    flag = str(env.get("SCCACHE_GHA_ENABLED", ""))
    assert flag, f"{workflow_name} {job_name} must set SCCACHE_GHA_ENABLED"
    assert "true" in flag, (
        f"{workflow_name} {job_name} must enable the backend, got {flag!r}"
    )


@pytest.mark.parametrize(("workflow_name", "job_name"), SCCACHE_CREDENTIAL_JOBS)
def test_the_export_precedes_setup_rust_and_the_server_start(
    workflow_name: str, job_name: str
) -> None:
    """Require the export before setup-rust and before any server start.

    On Ubicloud the runner re-injects the v2 service variables into every
    action step, so a server started inside `setup-rust` binds GitHub's
    service whatever the export said. Every job here passes
    `use-sccache: false` and starts the server from a `run` step after the
    export instead.
    """
    steps = job_steps(load_workflow(WORKFLOW_DIR / workflow_name), job_name)
    export = next(
        index
        for index, step in enumerate(steps)
        if str(step.get("uses", "")) == SCCACHE_CREDENTIALS_ACTION
    )
    setup = next(
        index
        for index, step in enumerate(steps)
        if SETUP_RUST_ACTION in str(step.get("uses", ""))
    )
    assert export < setup, (
        f"{workflow_name} {job_name} must export before the toolchain setup"
    )
    inputs = require_mapping(steps[setup].get("with"), "Setup Rust inputs")
    assert inputs.get("use-sccache") == "false", (
        f"{workflow_name} {job_name} must not let setup-rust start the server"
    )
    starts = [
        index
        for index, step in enumerate(steps)
        if "sccache --zero-stats" in str(step.get("run", ""))
    ]
    for start in starts:
        assert export < start, (
            f"{workflow_name} {job_name} must export before starting the server"
        )
