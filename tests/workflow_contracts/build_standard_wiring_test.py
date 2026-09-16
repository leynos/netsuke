"""Contract tests for the build standard's reach across the workflows.

``.cargo/config.toml`` puts every dev-profile build on the Cranelift backend
and, on Linux, on the pinned ``mold`` linker. That file is auto-discovered, so
the setting reaches lanes nobody edited: a job that compiles on the dev profile
without the Cranelift component fails to build at all, and ``dev-fast-check``
gates the Make targets on it besides.

Two exclusions run the other way. ``cargo llvm-cov`` needs LLVM source-based
instrumentation, which Cranelift does not emit, so the coverage steps must
override the backend; a measured build is also a reproducibility claim, so the
parallel frontend stays out of it.

These tests assert the command each lane runs and the environment each coverage
step declares, not a step name: renaming a step must not be able to satisfy
them, and deleting the install or the override must not be able to pass.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
from workflow_loading import (
    CI_WINDOWS_WORKFLOW_PATH,
    CI_WORKFLOW_PATH,
    COVERAGE_MAIN_WORKFLOW_PATH,
    MUTATION_TESTING_WORKFLOW_PATH,
    REPO_ROOT,
    job_steps,
    load_workflow,
    require_mapping,
    step_runs,
)

if typ.TYPE_CHECKING:
    from pathlib import Path

#: The command that installs the Cranelift component and the pinned linker.
INSTALL_COMMAND = "install-dev-fast"

#: The Cargo environment override that takes a build back to the LLVM backend.
BACKEND_OVERRIDE = "CARGO_PROFILE_DEV_CODEGEN_BACKEND"

#: Flags no measured build may carry. Cranelift cannot emit the coverage
#: instrumentation at all, and the parallel frontend is excluded from builds
#: whose output is a measurement.
EXCLUDED_FLAGS = ("cranelift", "-Zthreads")

NETSUKEFILE_WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "netsukefile-test.yml"

#: Every job that compiles on the dev profile and therefore needs the backend
#: installed before its first build.
DEV_PROFILE_JOBS = (
    (CI_WORKFLOW_PATH, "build-test"),
    (CI_WINDOWS_WORKFLOW_PATH, "lint-windows"),
    (CI_WINDOWS_WORKFLOW_PATH, "build-test-windows"),
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


@pytest.mark.parametrize(("path", "job"), DEV_PROFILE_JOBS, ids=str)
def test_dev_profile_jobs_install_the_build_standard(path: Path, job: str) -> None:
    """Assert each compiling job runs the installer before Cargo sees the tree.

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
        f"{path.name} job {job} must run `make {INSTALL_COMMAND}`; its dev-profile "
        f"builds are gated on the Cranelift component, got {runs!r}"
    )
    assert all(command.strip().startswith("make") for command in matching), (
        f"{path.name} job {job} must install through the Make target, so the "
        f"remedy a failed capability check names is the command CI runs, "
        f"got {matching!r}"
    )


def test_the_mutation_lane_installs_the_standard_before_mutating() -> None:
    """Assert cargo-mutants gets the backend through the shared setup hook.

    The mutation run is a reusable workflow this repository only calls, so its
    only channel for extra provisioning is the ``setup-commands`` input. Without
    it every mutant fails to compile and the run reports nothing useful.
    """
    workflow = load_workflow(MUTATION_TESTING_WORKFLOW_PATH)
    job = require_mapping(
        require_mapping(workflow.get("jobs"), "mutation-testing jobs").get("mutation"),
        "mutation job",
    )
    inputs = require_mapping(job.get("with"), "mutation job inputs")
    setup = inputs.get("setup-commands")
    assert isinstance(setup, str), (
        f"the mutation lane must declare setup-commands as a string, got {setup!r}"
    )
    assert INSTALL_COMMAND in setup, (
        "the mutation lane must pass `make install-dev-fast` as setup-commands, "
        f"got {setup!r}"
    )


@pytest.mark.parametrize(("path", "job"), COVERAGE_STEPS, ids=str)
def test_coverage_steps_take_the_backend_back_to_llvm(path: Path, job: str) -> None:
    """Assert every coverage step overrides the backend to LLVM."""
    steps = _coverage_steps(path, job)
    assert steps, f"{path.name} job {job} should run the shared coverage action"
    for step in steps:
        env = require_mapping(step.get("env"), f"{path.name} coverage step env")
        assert env.get(BACKEND_OVERRIDE) == "llvm", (
            f"{path.name} job {job} must set {BACKEND_OVERRIDE} to llvm: Cranelift "
            f"emits no source-based instrumentation, got {env.get(BACKEND_OVERRIDE)!r}"
        )


@pytest.mark.parametrize(("path", "job"), COVERAGE_STEPS, ids=str)
def test_coverage_steps_carry_no_excluded_flag(path: Path, job: str) -> None:
    """Assert no coverage step reintroduces Cranelift or the parallel frontend.

    Separate from the override test because the two fail to different edits:
    the override can be present while a ``RUSTFLAGS`` value hands the parallel
    frontend back, and that would go unnoticed by an assertion about the
    backend alone.
    """
    for step in _coverage_steps(path, job):
        env = require_mapping(step.get("env"), f"{path.name} coverage step env")
        declared = " ".join(
            f"{key}={value}" for key, value in env.items() if key != BACKEND_OVERRIDE
        )
        for flag in EXCLUDED_FLAGS:
            assert flag not in declared, (
                f"{path.name} job {job} must not hand `{flag}` to a measured "
                f"build, got {declared!r}"
            )
