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

# ruff: ignore[suspicious-subprocess-import] - the step's own script is under test.
import subprocess
import typing as typ

from workflow_loading import (
    PACKAGE_WORKFLOW_PATH,
    RELEASE_WORKFLOW_PATH,
    REPO_ROOT,
    job_steps,
    load_workflow,
    named_step,
    require_mapping,
    workflow_job,
)

if typ.TYPE_CHECKING:
    from pathlib import Path

FLOOR_STEP = "Report the glibc floor"
LINUX_GATE = "inputs.platform == 'linux'"
BINARY = "target/${{ inputs.target }}/release/${BIN_NAME}"
TARGET = "x86_64-unknown-linux-gnu"
#: `readelf --version-info` output whose version needs top out at GLIBC_2.34
#: as versions, though GLIBC_2.9 sorts last as text. The symbol and definition
#: sections also name a GLIBC_2.99 that the binary defines and does not need.
READELF_FIXTURE = REPO_ROOT / "tests" / "data" / "readelf-version-info.txt"


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


def test_the_floor_is_the_highest_version_the_binary_needs(tmp_path: Path) -> None:
    """Run the step's script over fixed `readelf` output and read the summary.

    The floor is the greatest GLIBC version in the version-needs section,
    compared as a version rather than as text. A GLIBC_2.99 that appears only
    in the symbol and definition sections is no requirement, and reporting it
    would overstate the floor.
    """
    stubs = tmp_path / "bin"
    stubs.mkdir()
    readelf = stubs / "readelf"
    readelf.write_text(f'#!/bin/sh\nexec cat "{READELF_FIXTURE}"\n')
    readelf.chmod(0o755)
    summary = tmp_path / "summary.md"
    script = str(named_step(_steps(), FLOOR_STEP).get("run")).replace(
        "${{ inputs.target }}", TARGET
    )
    # The script is the workflow's own step with the target substituted, and
    # `readelf` resolves to the stub above; no untrusted input reaches it.
    # ruff: ignore[subprocess-without-shell-equals-true] - shell is False.
    result = subprocess.run(
        ["bash", "-c", script],  # ruff: ignore[start-process-with-partial-path] - resolved from the fixed PATH.
        check=False,
        env={
            "PATH": f"{stubs}:/usr/bin:/bin",
            "BIN_NAME": "netsuke",
            "GITHUB_STEP_SUMMARY": str(summary),
        },
        text=True,
        capture_output=True,
    )
    assert result.returncode == 0, f"the floor step failed: {result.stderr!r}"
    reported = summary.read_text()
    expected = f"- glibc floor for `{TARGET}`: `GLIBC_2.34`\n"
    assert reported == expected, (
        f"the summary must report the highest needed version, got {reported!r}"
    )
