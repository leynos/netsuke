"""Contract tests for the build standard's reach across the workflows.

``.cargo/config.toml`` puts every build on the `mold` linker and the parallel
`rustc` frontend. That file is auto-discovered, so a lane that builds through
``make`` links with a linker it must therefore have installed, and
``check-build-tools`` gates the Make targets on it besides.

Installing is only half of it. A lane that installed the tools after its first
build would link that build with the distribution linker and still report
success, so the install must also *precede* every step that compiles. Which
steps those are is derived rather than listed: the Makefile already declares,
through its ``check-build-tools`` prerequisites, exactly which targets need the
pinned tools, so that declaration is what this module follows.

Two exclusions run the other way. A build whose output is a measurement is a
reproducibility claim, so the coverage steps assign ``RUSTFLAGS`` themselves,
which displaces every ``rustflags`` table in the configuration.

These tests assert the command each lane runs and the environment each coverage
step declares, not a step name: renaming a step must not be able to satisfy
them, and deleting the install or the assignment must not be able to pass. The
predicates those assertions are built from live in
``build_standard_predicates.py``, so this module states the contract and that
one decides what the workflow text means.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
from build_standard_predicates import (
    CAPABILITY_TARGET,
    INSTALL_TARGET,
    compiles,
    gated_targets,
    installs_build_standard,
    only_coverage_steps,
    step_name,
)
from workflow_loading import (
    CI_WORKFLOW_PATH,
    COVERAGE_MAIN_WORKFLOW_PATH,
    MAKEFILE_PATH,
    REPO_ROOT,
    job_steps,
    load_workflow,
    require_mapping,
)

# `Path` is named only by annotations here, and `TC003` requires such an import
# to be guarded so the runtime module stays free of names it never evaluates.
# Nothing resolves these annotations: pytest collects without doing so and
# `ty check` reads them statically.
if typ.TYPE_CHECKING:
    from pathlib import Path

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


def _coverage_steps(path: Path, job: str) -> list[dict[str, object]]:
    """Return the steps in ``job`` that run the shared coverage action.

    Parameters
    ----------
    path
        Workflow file to read.
    job
        Job whose steps should be filtered.

    Returns
    -------
    list[dict[str, object]]
        Every step in the job that invokes the coverage action.
    """
    return only_coverage_steps(job_steps(load_workflow(path), job))


def _step(script: str, name: str = "step") -> dict[str, object]:
    """Build a synthetic step carrying just a ``run`` script."""
    return {"name": name, "run": script}


def test_the_install_predicate_requires_the_exact_goal() -> None:
    """Drive the exact-goal predicate against scripts the repository lacks.

    The repository runs the exact command in both linking jobs, so asserting
    only against them cannot show that the predicate distinguishes it from a
    near miss: a substring check would report the same True for every lane and
    nothing would fail. The near misses below are therefore the specification,
    and they are the shapes the repository has no example of.
    """
    assert installs_build_standard(_step(f"make {INSTALL_TARGET}")), (
        "the repository's own command must be recognised, or the lanes fail"
    )
    assert installs_build_standard(
        _step(f"set -euo pipefail\nmake {INSTALL_TARGET}")
    ), "a shell preamble is not a second goal"
    assert installs_build_standard(_step(f"make -j8 {INSTALL_TARGET}")), (
        "an option is not a goal, so the install still counts"
    )

    assert not installs_build_standard(_step(f"make {INSTALL_TARGET}-extra")), (
        "a longer goal is a different target, not the install step"
    )
    assert not installs_build_standard(
        _step(f"make {INSTALL_TARGET} {CAPABILITY_TARGET}")
    ), "two goals means the command is not the install and nothing else"
    assert not installs_build_standard(_step(f"make $({INSTALL_TARGET})")), (
        "a goal built by a nested invocation is not named as a goal"
    )
    assert not installs_build_standard(_step("cargo build")), (
        "a script that never invokes make cannot install anything"
    )
    assert not installs_build_standard({"name": "calls an action instead"}), (
        "a step with no run script installs nothing this contract can see"
    )


def test_the_compiling_predicate_separates_builds_from_queries() -> None:
    """Drive the build detector against the near misses of a compile.

    A detector that reported every step would pass the repository's ordering
    assertion for the wrong reason, and one that reported none would leave that
    assertion vacuous. Both directions are pinned here, on shapes chosen so the
    answer is not decided by the repository's own lanes.
    """
    gated = frozenset({"typecheck", "build"})

    assert compiles(_step("cargo build"), gated), "a bare cargo build compiles"
    assert compiles(_step("make typecheck"), gated), (
        "a gated Make target compiles through its recipe"
    )
    assert compiles(
        {"name": "Test and Measure Coverage", "uses": "x/generate-coverage@v1"},
        gated,
    ), "the coverage action compiles inside the action, with no run script"

    # The repository's own lanes open with a step that does nothing but
    # announce the toolchain, and a detector keyed on the driver's name would
    # count that step as the first build — failing every lane for printing a
    # version, and failing it for a change that is not a regression.
    assert not compiles(_step("rustc --version\ncargo --version"), gated), (
        "announcing a compiler version is not a build"
    )
    assert not compiles(_step("cargo tree"), gated), (
        "a query that lists dependencies is not a build"
    )
    assert not compiles(_step("cargo kani --version"), gated), (
        "a query is still a query when an option precedes the flag"
    )
    assert not compiles(_step("make check-fmt"), gated), (
        "an ungated Make target compiles nothing this contract is about"
    )
    # Cargo's name may appear as an operand rather than as a command. Reading
    # the words of a line instead of the command it runs would count this one.
    assert not compiles(_step("set -euo pipefail\nprintf '%s' 'cargo build'"), gated), (
        "a string that merely names a build is not one"
    )
    assert not compiles({"name": "no script at all", "with": {}}, gated), (
        "a step with neither coverage action nor run script compiles nothing"
    )
    assert not compiles(_step("set -euo pipefail\necho done"), gated), (
        "a step that runs no compiler is not a build"
    )


@pytest.mark.parametrize(("path", "job"), MOLD_LINKED_JOBS, ids=str)
def test_mold_linked_jobs_install_the_build_standard(path: Path, job: str) -> None:
    """Assert each linking job installs the pinned tools, once, before building.

    Asserting the command rather than a step name covers the first half: a lane
    that renamed the step but kept running it is fine, and one that kept the
    name while dropping the command is not.

    The ordering is the second half, and it is the half a name-based check
    cannot see. Installing after the first build leaves that build on the
    distribution linker while the lane still reports success, so the install's
    index is asserted to precede every step that can compile — including the
    coverage action, which compiles inside a composite action and declares no
    `run` script of its own.
    """
    makefile = MAKEFILE_PATH.read_text(encoding="utf-8")
    gated = gated_targets(makefile)
    # Without this the ordering assertion below is vacuous: an unparsed
    # Makefile would report no gated targets and every lane would pass having
    # checked nothing.
    assert {CAPABILITY_TARGET, "typecheck"} <= gated, (
        f"the Makefile must gate at least {CAPABILITY_TARGET!r} and 'typecheck' "
        f"on the capability check, found {sorted(gated)!r}"
    )

    steps = job_steps(load_workflow(path), job)
    installs = [
        index for index, step in enumerate(steps) if installs_build_standard(step)
    ]
    assert len(installs) == 1, (
        f"{path.name} job {job} must run exactly `make {INSTALL_TARGET}` once; "
        f"found it at step indices {installs!r}"
    )
    install_index = installs[0]

    builders = [
        (index, step_name(step))
        for index, step in enumerate(steps)
        if compiles(step, gated)
    ]
    assert builders, (
        f"{path.name} job {job} installs the build standard but compiles "
        f"nothing, so there is no ordering left to check"
    )
    late = [entry for entry in builders if entry[0] < install_index]
    assert not late, (
        f"{path.name} job {job} compiles before `make {INSTALL_TARGET}` at step "
        f"{install_index}, so those builds link with the distribution linker: "
        f"{late!r}"
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
