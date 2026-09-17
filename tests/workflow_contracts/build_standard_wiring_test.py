"""Contract tests for the build standard's reach across the workflows.

``.cargo/config.toml`` puts every build on the `mold` linker and the parallel
`rustc` frontend. That file is auto-discovered, so a lane that builds through
``make`` links with a linker it must therefore have installed, and
``check-build-tools`` gates the Make targets on it besides.

Two exclusions run the other way. A build whose output is a measurement is a
reproducibility claim, so the coverage steps assign ``RUSTFLAGS`` themselves,
which displaces every ``rustflags`` table in the configuration.

These tests assert the command each lane runs and the environment each coverage
step declares, not a step name: renaming a step must not be able to satisfy
them, and deleting the install or the assignment must not be able to pass.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
from workflow_loading import (
    CI_WORKFLOW_PATH,
    COVERAGE_MAIN_WORKFLOW_PATH,
    REPO_ROOT,
    job_steps,
    load_workflow,
    require_mapping,
    step_runs,
)

if typ.TYPE_CHECKING:
    from pathlib import Path

#: The command that installs the pinned linker and toolchain.
INSTALL_COMMAND = "install-build-tools"

#: Flags no measured build may carry. The parallel frontend is excluded because
#: a measurement is a reproducibility claim; the linker change goes with it so
#: the coverage lane needs no tool the other lanes install.
EXCLUDED_FLAGS = ("-Zthreads", "-fuse-ld=")

NETSUKEFILE_WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "netsukefile-test.yml"

#: Every job whose builds take the linker flag, and which therefore needs the
#: pinned linker installed before its first build. The Windows jobs are absent
#: deliberately: `mold` is Linux-only, so the `cfg` gate in the configuration
#: leaves the flag inert there and an install step would provision nothing.
MOLD_LINKED_JOBS = (
    (CI_WORKFLOW_PATH, "build-test"),
    (NETSUKEFILE_WORKFLOW_PATH, "netsukefile"),
)

#: Every step that produces a coverage measurement, by workflow and job.
COVERAGE_STEPS = (
    (CI_WORKFLOW_PATH, "build-test"),
    (COVERAGE_MAIN_WORKFLOW_PATH, "coverage-upload"),
)


def _uses_coverage_action(step: dict[str, object]) -> bool:
    """Return whether ``step`` calls the shared coverage action."""
    uses = step.get("uses")
    return isinstance(uses, str) and "generate-coverage" in uses


def _coverage_steps(path: Path, job: str) -> list[dict[str, object]]:
    """Return the steps in ``job`` that run the shared coverage action.

    Returns
    -------
    list[dict[str, object]]
        Every step in the job that invokes the coverage action.
    """
    return [
        step
        for step in job_steps(load_workflow(path), job)
        if _uses_coverage_action(step)
    ]


@pytest.mark.parametrize(("path", "job"), MOLD_LINKED_JOBS, ids=str)
def test_mold_linked_jobs_install_the_build_standard(path: Path, job: str) -> None:
    """Assert each linking job runs the installer before Cargo sees the tree.

    Asserting the command rather than a step name is the point: a lane that
    renamed the step but kept running it is fine, and one that kept the name
    while dropping the command is not.
    """
    runs = [
        command
        for command in step_runs(job_steps(load_workflow(path), job))
        if isinstance(command, str)
    ]
    matching = [command for command in runs if INSTALL_COMMAND in command]
    assert matching, (
        f"{path.name} job {job} must run `make {INSTALL_COMMAND}`; its builds link "
        f"with the pinned mold, got {runs!r}"
    )
    assert all(command.strip().startswith("make") for command in matching), (
        f"{path.name} job {job} must install through the Make target, so the "
        f"remedy a failed capability check names is the command CI runs, "
        f"got {matching!r}"
    )


@pytest.mark.parametrize(("path", "job"), COVERAGE_STEPS, ids=str)
def test_coverage_steps_assign_their_own_rustflags(path: Path, job: str) -> None:
    """Assert every coverage step assigns ``RUSTFLAGS`` at the step itself.

    Inheriting the value from the toolchain action would work, but it would put
    the exclusion out of sight of the step it protects, and nothing would fail
    if that action's default changed.
    """
    steps = _coverage_steps(path, job)
    assert steps, f"{path.name} job {job} should run the shared coverage action"
    for step in steps:
        env = require_mapping(step.get("env"), f"{path.name} coverage step env")
        rustflags = env.get("RUSTFLAGS")
        assert isinstance(rustflags, str), (
            f"{path.name} job {job} must assign RUSTFLAGS at the coverage step, "
            f"got {rustflags!r}"
        )
        assert "-D warnings" in rustflags, (
            f"{path.name} job {job} must keep warnings denied while measuring, "
            f"got {rustflags!r}"
        )


@pytest.mark.parametrize(("path", "job"), COVERAGE_STEPS, ids=str)
def test_coverage_steps_carry_no_excluded_flag(path: Path, job: str) -> None:
    """Assert no coverage step reintroduces an excluded flag.

    Separate from the assignment test because the two fail to different edits:
    the assignment can be present while its value hands the parallel frontend
    back, and that would go unnoticed by an assertion that only checks a value
    exists.
    """
    for step in _coverage_steps(path, job):
        env = require_mapping(step.get("env"), f"{path.name} coverage step env")
        declared = " ".join(f"{key}={value}" for key, value in env.items())
        for flag in EXCLUDED_FLAGS:
            assert flag not in declared, (
                f"{path.name} job {job} must not hand `{flag}` to a measured "
                f"build, got {declared!r}"
            )
