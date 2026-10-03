"""The Linux release lanes report their glibc floor on every run.

A Linux binary's highest required GLIBC symbol version is its portability
floor, and it follows the image the binary was linked on: v0.1.0-beta3
measured GLIBC_2.39 for x86_64, built natively on Ubuntu 24.04, and
GLIBC_2.18 for aarch64, built through `cross`. `build-and-package.yml`
writes each Linux binary's floor to the job summary after the build, so a
change to either image shows up in the release run rather than in a user's
bug report. The step is Linux-only, because macOS and Windows binaries carry
no glibc version. The script's fixture-backed behaviour tests live under
`scripts/tests`; this module holds the workflow wiring.

Run via ``make test-workflow-contracts``.
"""

from workflow_loading import (
    PACKAGE_WORKFLOW_PATH,
    RELEASE_WORKFLOW_PATH,
    job_steps,
    load_workflow,
    named_step,
    require_mapping,
    step_index_by_key,
    workflow_job,
)

FLOOR_STEP = "Report the glibc floor"
LINUX_GATE = "inputs.platform == 'linux'"


def _steps() -> list[dict[str, object]]:
    """Return the packaging job's steps."""
    return job_steps(load_workflow(PACKAGE_WORKFLOW_PATH), "build")


def test_the_floor_is_reported_for_linux_only() -> None:
    """Gate the report on the Linux platform, compared whole."""
    step = named_step(_steps(), FLOOR_STEP)
    assert step.get("if") == LINUX_GATE, (
        f"{FLOOR_STEP} must run exactly when {LINUX_GATE}, got {step.get('if')!r}"
    )


def test_the_floor_is_read_after_the_build() -> None:
    """Read the binary only once the build step has produced it."""
    steps = _steps()
    names = [str(step.get("name", "")) for step in steps]
    assert names.index("Build release binary") < names.index(FLOOR_STEP), (
        f"{FLOOR_STEP} must follow the build, got step order {names!r}"
    )


def test_the_floor_script_receives_inputs_through_the_environment() -> None:
    """Use the tested Python script with explicit target and binary inputs."""
    steps = _steps()
    step = named_step(steps, FLOOR_STEP)
    assert step.get("run") == (
        'uv run --no-project --python "$UV_PYTHON" scripts/report_glibc_floor.py'
    ), "the glibc logic must run through its tested Python script"
    env = require_mapping(step.get("env"), f"{FLOOR_STEP} environment")
    assert env.get("INPUT_TARGET") == "${{ inputs.target }}", (
        "the target must reach the script through INPUT_TARGET"
    )
    assert env.get("INPUT_BIN_NAME") == "${{ env.BIN_NAME }}", (
        "the binary name must reach the script through INPUT_BIN_NAME"
    )
    floor_index = step_index_by_key(steps, "name", FLOOR_STEP)
    setup_index = step_index_by_key(steps, "uses", "setup-uv")
    assert setup_index < floor_index, (
        "uv must be installed before the floor script runs"
    )
    setup = require_mapping(steps[setup_index].get("with"), "setup-uv inputs")
    assert setup.get("python-version") == "${{ inputs['python-version'] }}", (
        "the script must use the Python version configured by the reusable workflow"
    )


def test_a_linux_release_passes_the_platform_the_gate_names() -> None:
    """Keep the gate reachable: the Linux release build passes that platform."""
    job = workflow_job(load_workflow(RELEASE_WORKFLOW_PATH), "build-linux")
    inputs = require_mapping(job.get("with"), "build-linux inputs")
    assert inputs.get("platform") == "linux", (
        f"build-linux must pass platform linux, got {inputs.get('platform')!r}"
    )
