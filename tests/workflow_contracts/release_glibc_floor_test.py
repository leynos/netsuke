"""The Linux release lanes report their glibc floor on every run.

A Linux binary's highest required GLIBC symbol version is its portability
floor, and it follows the image the binary was linked on: v0.1.0-beta3
measured GLIBC_2.39 for x86_64, built natively on Ubuntu 24.04, and
GLIBC_2.18 for aarch64, built through `cross`. `build-and-package.yml`
writes each Linux binary's floor to the job summary after the build, so a
change to either image shows up in the release run rather than in a user's
bug report. The step is Linux-only, because macOS and Windows binaries carry
no glibc version, and `readelf` reads the aarch64 binary on the x64 runner
without a multi-architecture binutils.

Run via ``make test-workflow-contracts``.
"""

from workflow_loading import (
    PACKAGE_WORKFLOW_PATH,
    RELEASE_WORKFLOW_PATH,
    job_steps,
    load_workflow,
    named_step,
    require_mapping,
    workflow_job,
)

FLOOR_STEP = "Report the glibc floor"
LINUX_GATE = "inputs.platform == 'linux'"
BINARY = "target/${{ inputs.target }}/release/${BIN_NAME}"


def _steps() -> list[dict[str, object]]:
    """Return the packaging job's steps."""
    return job_steps(load_workflow(PACKAGE_WORKFLOW_PATH), "build")


def _command_lines(run: object) -> list[str]:
    """Return the non-blank, non-comment lines of a run block, stripped."""
    return [
        line.strip()
        for line in str(run or "").splitlines()
        if line.strip() and not line.strip().startswith("#")
    ]


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


def test_the_floor_is_read_from_the_built_binary_into_the_summary() -> None:
    """Read version needs from the target's binary and write them out.

    `readelf --version-info` rather than `objdump -T`, because the x64 runner's
    binutils cannot necessarily read the aarch64 ELF, and the value must reach
    the job summary, where a reviewer of the release run sees it.
    """
    lines = _command_lines(named_step(_steps(), FLOOR_STEP).get("run"))
    assert f'binary="{BINARY}"' in lines, (
        f"{FLOOR_STEP} must name the target's release binary, got {lines!r}"
    )
    assert any('readelf --version-info "${binary}"' in line for line in lines), (
        f"{FLOOR_STEP} must read the binary's version needs with readelf: {lines!r}"
    )
    assert any('>> "$GITHUB_STEP_SUMMARY"' in line for line in lines), (
        f"{FLOOR_STEP} must write the floor to the job summary, got {lines!r}"
    )


def test_a_linux_release_passes_the_platform_the_gate_names() -> None:
    """Keep the gate reachable: the Linux release build passes that platform."""
    job = workflow_job(load_workflow(RELEASE_WORKFLOW_PATH), "build-linux")
    inputs = require_mapping(job.get("with"), "build-linux inputs")
    assert inputs.get("platform") == "linux", (
        f"build-linux must pass platform linux, got {inputs.get('platform')!r}"
    )
