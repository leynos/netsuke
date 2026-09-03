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
