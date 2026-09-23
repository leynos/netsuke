"""The Linux release lanes compile through sccache, and no other lane does.

`build-and-package.yml` keeps the compiler cache out of its job environment
for every lane. Windows overflows its command line under sccache, and macOS
runs on GitHub-hosted runners that the Ubicloud proxy does not serve. Both
Linux targets build natively on Ubicloud, each on its own architecture's
runner, so neither goes through `cross`'s container, and both opt in with the
merge gate's proxy wiring, step by step, every step gated on the Linux
platform. The pull-request dry run runs both on every push, and a repeat push
can read its own branch's cache scope.

Gating steps rather than the environment is deliberate. An earlier shape
exempted Windows through a negated environment expression and got the negation
backwards once, which cost a release build. Each step here carries the same
positive condition, compared whole, and the caller is held to passing the
platform that satisfies it, so the steps can actually run.

Run via ``make test-workflow-contracts``.
"""

import pytest
from action_references import require_external_action_sha
from cache_contract_data import WORKFLOW_DIR
from workflow_loading import (
    RELEASE_WORKFLOW_PATH,
    job_steps,
    load_workflow,
    named_step,
    require_list,
    require_mapping,
    workflow_job,
)

#: The platform whose lanes compile through sccache: both Linux targets build
#: natively on Ubicloud, each on its own architecture's runner.
TARGET_GATE = "inputs.platform == 'linux'"

#: The opt-in steps, in the order they must run, each gated on the target.
GATED_STEPS = (
    "Enable sccache",
    "Export sccache credentials",
    "Install sccache",
    "Reset sccache statistics",
)


def _package_steps() -> list[dict[str, object]]:
    """Return the packaging job's steps."""
    return job_steps(load_workflow(WORKFLOW_DIR / "build-and-package.yml"), "build")


@pytest.mark.parametrize("step_name", GATED_STEPS)
def test_each_opt_in_step_is_gated_on_the_linux_platform(step_name: str) -> None:
    """Run every opt-in step for the Linux lanes and for nothing else."""
    step = named_step(_package_steps(), step_name)
    assert step.get("if") == TARGET_GATE, (
        f"{step_name} must run exactly when {TARGET_GATE}, got {step.get('if')!r}"
    )


def test_the_statistics_are_reported_for_the_same_lane_even_on_failure() -> None:
    """Report the counters on the cached lane whether or not the build passed."""
    step = named_step(_package_steps(), "Show sccache statistics")
    assert step.get("if") == f"always() && {TARGET_GATE}", (
        f"statistics must be shown on failure too, for the cached lane only, "
        f"got {step.get('if')!r}"
    )
    assert "sccache --show-stats --stats-format=json" in str(step.get("run")), (
        "the cached lane must export machine-readable statistics"
    )


def test_the_opt_in_runs_in_order_around_the_build() -> None:
    """Enable, export, install and reset before the build; report after it.

    The export's verification reads `SCCACHE_GHA_ENABLED`, so the variable is
    written first. A server started before the export stays on local disk for
    the whole job, so the reset, which starts it, follows the export.
    """
    steps = _package_steps()
    names = [str(step.get("name", "")) for step in steps]
    order = [*GATED_STEPS, "Build release binary", "Show sccache statistics"]
    positions = [names.index(name) for name in order]
    assert positions == sorted(positions), (
        f"the opt-in must run as {order}, got step order {names!r}"
    )


def test_the_opt_in_writes_the_wrapper_and_the_backend_together() -> None:
    """Write both variables, so the wrapper never runs without its backend."""
    run = str(named_step(_package_steps(), "Enable sccache").get("run", ""))
    for assignment in ("RUSTC_WRAPPER=sccache", "SCCACHE_GHA_ENABLED=true"):
        assert assignment in run, f"Enable sccache must write {assignment}"
    assert '>> "$GITHUB_ENV"' in run, "the assignments must reach later steps"


def test_the_opt_in_uses_the_proxy_export_and_the_pinned_binary() -> None:
    """Use the gate's proxy export and the gate's exact sccache release."""
    steps = _package_steps()
    export = named_step(steps, "Export sccache credentials")
    assert export.get("uses") == "./.github/actions/sccache-gha-credentials", (
        "the cached lane must export the Ubicloud proxy address, got "
        f"{export.get('uses')!r}"
    )
    install = named_step(steps, "Install sccache")
    require_external_action_sha(
        install.get("uses"), "taiki-e/install-action", "the release lane's sccache"
    )
    inputs = require_mapping(install.get("with"), "sccache installer inputs")
    assert inputs == {"tool": "sccache@0.16.0", "fallback": "none"}, (
        f"the release lane must install the gate's sccache release, got {inputs!r}"
    )
    build = require_mapping(
        named_step(steps, "Build release binary").get("with"), "build inputs"
    )
    assert build.get("use-sccache") == "false", (
        "the nested action's own sccache would bypass the proxy"
    )


def test_the_release_workflow_passes_the_cached_target_to_a_linux_lane() -> None:
    """Hold the gate reachable: a Linux build passes the platform it names.

    A condition no caller can satisfy would leave every opt-in step dead while
    this contract stayed green.
    """
    job = workflow_job(load_workflow(RELEASE_WORKFLOW_PATH), "build-linux")
    with_ = require_mapping(job.get("with"), "build-linux inputs")
    assert with_.get("platform") == "linux", "build-linux must build for Linux"
    assert with_.get("target") == "${{ matrix.target }}", (
        f"build-linux must pass its matrix target, got {with_.get('target')!r}"
    )
    strategy = require_mapping(job.get("strategy"), "build-linux strategy")
    matrix = require_mapping(strategy.get("matrix"), "build-linux matrix")
    targets = [
        require_mapping(entry, "matrix entry").get("target")
        for entry in require_list(matrix.get("include"), "matrix include")
    ]
    assert targets, f"build-linux must build at least one target, got {targets!r}"
