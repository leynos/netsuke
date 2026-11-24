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


def _coverage_steps(path: object, job: str) -> list[dict[str, object]]:
    """Return the steps in ``job`` that run the shared coverage action."""
    return only_coverage_steps(job_steps(load_workflow(path), job))


@pytest.mark.parametrize(("path", "job"), COVERAGE_STEPS, ids=str)
def test_coverage_steps_opt_the_linker_wrapper_out(path: object, job: str) -> None:
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
def test_packaging_jobs_opt_the_linker_wrapper_out(path: object, job: str) -> None:
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


@pytest.mark.parametrize(("path", "job"), HELPER_BUILD_JOBS, ids=str)
def test_helper_build_jobs_provision_the_build_standard(path: object, job: str) -> None:
    """Assert each helper-building Linux job installs the pinned standard.

    These jobs ship an artefact another tool links, but they still compile a
    helper in the checkout, where the configuration's ``linker`` key applies.
    Asserting only the opt-out would pass while the nested build had no mold to
    fall back on, which is exactly how a packaging run failed; asserting the
    install as well makes the lane correct whichever branch a nested tool takes.
    """
    steps = job_steps(load_workflow(path), job)
    installs = [step for step in steps if installs_build_standard(step)]
    assert len(installs) == 1, (
        f"{path.name} job {job} must run exactly one `make {INSTALL_TARGET}` "
        f"step, found {len(installs)}"
    )
