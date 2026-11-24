"""Contracts for the linker opt-out and helper-build provisioning.

The configuration's ``linker`` key is not displaced by an assigned
``RUSTFLAGS``, so the build standard's exclusions cannot rely on displacement
for it. Each excluded shape therefore sets ``NETSUKE_PLATFORM_LINKER=1``, and
each lane that compiles a helper in the checkout provisions the pinned standard
so a nested build is correct whichever branch it takes.

Split from ``build_standard_wiring_test.py`` to keep both modules inside the
repository's file-length rule; that module owns the install-ordering contracts
and this one owns the exclusion and fallback contracts.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
from build_standard_predicates import (
    INSTALL_TARGET,
    installs_build_standard,
    only_coverage_steps,
)
from workflow_loading import (
    CI_WORKFLOW_PATH,
    COVERAGE_MAIN_WORKFLOW_PATH,
    PACKAGE_WORKFLOW_PATH,
    RELEASE_WORKFLOW_PATH,
    job_steps,
    load_workflow,
    require_mapping,
    workflow_job,
)

# `Path` is named only by annotations here, and `TC003` requires such an import
# to be guarded so the runtime module stays free of names it never evaluates.
# Nothing resolves these annotations: pytest collects without doing so and
# `ty check` reads them statically.
if typ.TYPE_CHECKING:
    from pathlib import Path

#: The variable the linker wrapper reads to select the platform linker.
PLATFORM_LINKER_ENV = "NETSUKE_PLATFORM_LINKER"

#: Every step that produces a coverage measurement, by workflow and job.
COVERAGE_STEPS = (
    (CI_WORKFLOW_PATH, "build-test"),
    (COVERAGE_MAIN_WORKFLOW_PATH, "coverage-upload"),
)

#: Jobs whose builds ship an artefact and are therefore excluded from the
#: standard, so they must also opt the linker wrapper out.
PACKAGING_JOBS = (
    (PACKAGE_WORKFLOW_PATH, "build"),
    (RELEASE_WORKFLOW_PATH, "package-crate"),
)

#: Jobs that compile a helper *in the checkout* on a Linux runner while the
#: configuration's `linker` key is active, and therefore need the pinned
#: standard present even though the artefact they ship is built elsewhere.
#:
#: The packaging job is here because `Generate release help` runs a nested
#: Cargo build under `cargo-orthohelp`. Its job-level `NETSUKE_PLATFORM_LINKER`
#: is meant to send that build to the platform linker, but the variable is not
#: guaranteed to reach a nested tool, and a run that fell through to mold on a
#: runner without it failed with `collect2: fatal error: cannot find 'ld'`.
#: Provisioning the standard makes the lane correct either way, so the property
#: asserted is presence, not ordering: the release binary itself is linked by
#: `cross` in its own container before this matters.
HELPER_BUILD_JOBS = ((PACKAGE_WORKFLOW_PATH, "build"),)


def _coverage_steps(path: Path, job: str) -> list[dict[str, object]]:
    """Return the steps in ``job`` that run the shared coverage action."""
    return only_coverage_steps(job_steps(load_workflow(path), job))


@pytest.mark.parametrize(("path", "job"), COVERAGE_STEPS, ids=str)
def test_coverage_steps_opt_the_linker_wrapper_out(path: Path, job: str) -> None:
    """Assert every coverage step keeps the platform linker.

    ``RUSTFLAGS`` no longer carries the whole exclusion: the configuration's
    ``linker`` key survives an assigned value, so a coverage step that only
    assigned ``RUSTFLAGS`` would still link through ``mold``. The opt-out is
    asserted separately from the flag assertions because the two fail to
    different edits: dropping the opt-out leaves every flag assertion passing.
    """
    for step in _coverage_steps(path, job):
        env = require_mapping(step.get("env"), f"{path.name} coverage step env")
        assert str(env.get(PLATFORM_LINKER_ENV)) == "1", (
            f"{path.name} job {job} must set {PLATFORM_LINKER_ENV}=1 on the "
            f"coverage step, or the measured build still links with mold; got "
            f"{env.get(PLATFORM_LINKER_ENV)!r}"
        )


@pytest.mark.parametrize(("path", "job"), PACKAGING_JOBS, ids=str)
def test_packaging_jobs_opt_the_linker_wrapper_out(path: Path, job: str) -> None:
    """Assert each packaging job keeps the platform linker.

    A shipped artefact is one of the standard's exclusions. Its job already
    exports ``RUSTFLAGS`` through the toolchain action, which displaces the
    configuration's ``rustflags`` tables, but not its ``linker`` key; without
    this variable the artefact would still link with ``mold``.
    """
    env = require_mapping(
        workflow_job(load_workflow(path), job).get("env"),
        f"{path.name} jobs.{job} env",
    )
    assert str(env.get(PLATFORM_LINKER_ENV)) == "1", (
        f"{path.name} job {job} must set {PLATFORM_LINKER_ENV}=1, or the "
        f"packaged artefact links with mold; got "
        f"{env.get(PLATFORM_LINKER_ENV)!r}"
    )


#: The step whose nested Cargo build the packaging lane's install protects.
PROTECTED_STEP = "Generate release help"

#: The condition the install must carry, so a Windows lane is untouched.
LINUX_CONDITION = "inputs.platform == 'linux'"

#: The export that puts the installed prefix on later steps' `PATH`. Without
#: it the install exists but the nested build cannot find `mold`.
PATH_EXPORT = "${HOME}/.local/bin"


def helper_build_provisioning_problems(steps: list[dict[str, object]]) -> list[str]:
    """Return every way ``steps`` fails the helper-build provisioning contract.

    Three properties, because each fails a different edit. A count alone
    accepts an install placed *after* the step it protects, an install gated to
    the wrong platform, and an install that never extends ``PATH`` — the last
    being a failure mode this lane actually produced, where the tools were
    present but the nested build could not resolve them.

    Kept separate from the assertion so a seeded edit can be checked against
    the same predicate: a guard that cannot fail on the defect it names is not
    a guard.

    Parameters
    ----------
    steps
        The job's parsed steps, in declaration order.

    Returns
    -------
    list[str]
        One entry per unmet property, or an empty list when the contract holds.
    """
    installs = [
        index for index, step in enumerate(steps) if installs_build_standard(step)
    ]
    if len(installs) != 1:
        return [
            f"expected exactly one `make {INSTALL_TARGET}` step, found {len(installs)}"
        ]

    install = steps[installs[0]]
    problems: list[str] = []

    condition = install.get("if")
    if condition != LINUX_CONDITION:
        problems.append(
            f"the install must be gated on `{LINUX_CONDITION}`, got {condition!r}"
        )

    script = install.get("run")
    if not isinstance(script, str) or PATH_EXPORT not in script:
        problems.append(
            f"the install must append `{PATH_EXPORT}` to `$GITHUB_PATH`, got {script!r}"
        )

    protected = [
        index for index, step in enumerate(steps) if step.get("name") == PROTECTED_STEP
    ]
    if not protected:
        problems.append(f"the job must declare a `{PROTECTED_STEP}` step")
    elif installs[0] > protected[0]:
        problems.append(
            f"the install must precede `{PROTECTED_STEP}` (install at "
            f"{installs[0]}, protected step at {protected[0]})"
        )

    return problems


@pytest.mark.parametrize(("path", "job"), HELPER_BUILD_JOBS, ids=str)
def test_helper_build_jobs_provision_the_build_standard(path: Path, job: str) -> None:
    """Assert each helper-building Linux job provisions and exposes the standard.

    These jobs ship an artefact another tool links, but they still compile a
    helper in the checkout, where the configuration's ``linker`` key applies.
    Asserting only the opt-out would pass while the nested build had no mold to
    fall back on, which is exactly how a packaging run failed; asserting the
    install, its platform gate, its ``PATH`` export and its position makes the
    lane correct whichever branch a nested tool takes.
    """
    problems = helper_build_provisioning_problems(job_steps(load_workflow(path), job))
    assert not problems, f"{path.name} job {job} violates the contract: {problems}"


def _install_step(**overrides: object) -> dict[str, object]:
    """Build a synthetic install step satisfying the contract."""
    step: dict[str, object] = {
        "name": "Install the build standard",
        "if": LINUX_CONDITION,
        "run": f'make {INSTALL_TARGET}\necho "{PATH_EXPORT}" >> "$GITHUB_PATH"',
    }
    step.update(overrides)
    return step


def test_the_helper_build_contract_rejects_each_defect_it_names() -> None:
    """Drive the provisioning predicate against the defects it exists to catch.

    The repository's own lane satisfies every property, so asserting only
    against it cannot show the predicate distinguishes a correct lane from a
    near miss. Each fixture below is one edit away from correct, which is the
    shape a future change would take.
    """
    protected = {"name": PROTECTED_STEP, "run": "bash scripts/generate-release-help.sh"}
    correct = [_install_step(), protected]
    assert not helper_build_provisioning_problems(correct), (
        "the contract must accept a correct lane, or the real lane cannot pass"
    )

    assert helper_build_provisioning_problems([protected, _install_step()]), (
        "an install placed after the step it protects must fail"
    )
    assert helper_build_provisioning_problems([
        _install_step(**{"if": "inputs.platform == 'windows'"}),
        protected,
    ]), "an install gated to another platform must fail"
    assert helper_build_provisioning_problems([
        _install_step(run=f"make {INSTALL_TARGET}"),
        protected,
    ]), "an install that never extends PATH must fail"
    assert helper_build_provisioning_problems([protected]), (
        "a lane with no install at all must fail"
    )
    assert helper_build_provisioning_problems([_install_step()]), (
        "a lane with no protected step must fail, or the ordering check is vacuous"
    )
