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
    SCCACHE_LOCAL_DIR_JOBS,
    SCCACHE_WRAPPER_JOBS,
    WORKFLOW_DIR,
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


def test_packaging_installs_the_sccache_it_wraps() -> None:
    """Require the packaging lane to install the binary its wrapper names.

    `RUSTC_WRAPPER` without an installer is what produced "sccache: error:
    failed to spawn Command" on the Windows packaging build: the nested shared
    action installs sccache only along a path this caller does not take.
    """
    steps = job_steps(load_workflow(WORKFLOW_DIR / "build-and-package.yml"), "build")
    installer = named_step(steps, "Install sccache")
    assert installer.get("uses") == (
        "taiki-e/install-action@18b1216eba7f8039b0f8d131d5473787f0edce68"
    ), "the packaging lane must use the pinned sccache installer"
    inputs = require_mapping(installer.get("with"), "sccache installer inputs")
    assert inputs.get("tool") == "sccache@0.16.0", (
        "the packaging lane must install the exact tested release"
    )
    assert inputs.get("fallback") == "none", (
        "the packaging lane must not fall back to a source build"
    )
    build_index = steps.index(named_step(steps, "Build release binary"))
    assert steps.index(installer) < build_index, (
        "sccache must exist before the build that wraps it"
    )


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
