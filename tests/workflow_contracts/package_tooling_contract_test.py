"""Protect the package workflow's Python setup and staged-output boundaries.

Run via ``make test-workflow-contracts``.
"""

from workflow_loading import PACKAGE_WORKFLOW_PATH, load_workflow, require_mapping

#: The step that exports the MSVC toolchain ahead of nested Cargo builds.
MSVC_STEP_NAME = "Configure MSVC for nested Cargo builds"

#: Script fragments that make the MSVC step do its job. `VsDevCmd.bat` is what
#: discovers the toolchain, `kernel32.lib` is the probe that proves the Windows
#: SDK joined the search path, and the linker variable is what the nested build
#: actually consumes, because it forwards target linker overrides by name.
MSVC_SCRIPT_FRAGMENTS = (
    "VsDevCmd.bat",
    "kernel32.lib",
    "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER",
    "GITHUB_PATH",
)

#: The two steps on the build job that compile Rust, in the order they run.
#: `Build release binary` is the release artefact itself and must link with the
#: toolchain the release image expects; `Generate release help` is where the
#: nested bridge build runs, which is the one that regressed on Windows.
RUST_BUILDING_STEPS = ("Build release binary", "Generate release help")


def _build_steps() -> list[dict[str, object]]:
    """Return parsed steps from the reusable build job."""
    workflow = load_workflow(PACKAGE_WORKFLOW_PATH)
    jobs = require_mapping(workflow.get("jobs"), "build-and-package.yml jobs")
    build = require_mapping(jobs.get("build"), "build-and-package.yml build job")
    steps = build.get("steps")
    assert isinstance(steps, list), "build job steps must be a list"
    return [
        require_mapping(step, f"build-and-package.yml build step {index}")
        for index, step in enumerate(steps)
    ]


def test_build_job_installs_uv_for_all_platforms() -> None:
    """Keep setup-uv and its configured interpreter on every build platform."""
    setup_steps = [step for step in _build_steps() if step.get("name") == "Install uv"]
    assert len(setup_steps) == 1, "build job must have one Install uv step"
    setup = setup_steps[0]
    uses = setup.get("uses")
    assert isinstance(uses, str), "Install uv must declare its action"
    assert uses.startswith("astral-sh/setup-uv@"), (
        "Install uv must use the pinned astral-sh/setup-uv action"
    )
    inputs = require_mapping(setup.get("with"), "Install uv inputs")
    assert inputs.get("python-version") == "${{ inputs['python-version'] }}", (
        "Install uv must use the configured workflow Python version"
    )
    assert "if" not in setup, "Install uv must run on every build platform"


def test_build_job_does_not_reintroduce_capture_staged_paths() -> None:
    """Keep staged outputs available directly to later workflow steps."""
    steps = _build_steps()
    names = [step.get("name") for step in steps]
    identifiers = [step.get("id") for step in steps]
    assert "Capture staged paths" not in names, (
        "later steps must consume staged outputs directly"
    )
    assert "stage_paths" not in identifiers, (
        "the removed staged-output copy must not return under a different name"
    )


def test_build_job_configures_msvc_before_the_nested_cargo_build() -> None:
    """Keep the Windows MSVC toolchain configured for the nested bridge build.

    `cargo-orthohelp` 0.10.0 builds a bridge binary when it generates help, so
    `Generate release help` now runs a nested Cargo build that 0.9.x did not.
    On Windows that build resolves `link.exe` through `PATH`, and every Bash
    step on this lane starts with Git's `/usr/bin` ahead of Visual Studio,
    where coreutils' `link` answers instead and rejects the object-file
    operands. The failure surfaces only on the Windows lanes, after the release
    binary itself has already linked successfully, so nothing else here
    catches it.

    The step must also come *after* `Build release binary`, which looks like
    the opposite of the ordering rule but is not. The exported `INCLUDE` and
    `LIB` name x64 libraries, and that step cross-compiles the aarch64 Windows
    lane; leaving them in scope would let it link against x64 objects. Nothing
    is lost by the placement: `Build release binary` passes
    `skip-man-page-discovery: 'true'`, so `Generate release help` is the only
    step on this job that runs the nested build.
    """
    steps = _build_steps()
    msvc_indices = [
        index for index, step in enumerate(steps) if step.get("name") == MSVC_STEP_NAME
    ]
    assert len(msvc_indices) == 1, (
        f"build job must configure MSVC exactly once, got {msvc_indices!r}"
    )
    msvc = steps[msvc_indices[0]]
    assert msvc.get("if") == "inputs.platform == 'windows'", (
        f"MSVC configuration must stay Windows-only, got {msvc.get('if')!r}"
    )
    assert msvc.get("shell") == "pwsh", (
        f"MSVC configuration must run under pwsh, got {msvc.get('shell')!r}"
    )
    script = str(msvc.get("run", ""))
    for fragment in MSVC_SCRIPT_FRAGMENTS:
        assert fragment in script, (
            f"MSVC configuration must keep {fragment!r} in its script"
        )

    positions = {str(step.get("name")): index for index, step in enumerate(steps)}
    for name in (*RUST_BUILDING_STEPS, MSVC_STEP_NAME):
        assert name in positions, (
            f"build job must keep its {name!r} step, so the ordering checks "
            f"below have something to compare against; got {list(positions)}"
        )
    assert positions[MSVC_STEP_NAME] > positions["Build release binary"], (
        "MSVC configuration must follow the release build, whose x64 library "
        f"paths would otherwise reach the aarch64 cross-build; got MSVC at "
        f"{positions[MSVC_STEP_NAME]} and the release build at "
        f"{positions['Build release binary']}"
    )
    assert positions[MSVC_STEP_NAME] < positions["Generate release help"], (
        "MSVC configuration must precede the nested bridge build, got MSVC at "
        f"{positions[MSVC_STEP_NAME]} and release help at "
        f"{positions['Generate release help']}"
    )
