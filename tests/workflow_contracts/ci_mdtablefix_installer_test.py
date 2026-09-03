"""Hold the Markdown formatter installer's version and source-build contract.

`make check-fmt` shells out to `mdtablefix`, so both formatter jobs install a
pinned release before running it. They must replace a stale executable and
must never fall back to a source build: `cargo-binstall`'s default strategy
list ends in `compile`, so a missing prebuilt artefact would otherwise be
compiled in CI.

Run via ``make test-workflow-contracts``.
"""

import pytest
from workflow_loading import SETUP_RUST_JOBS, job_steps, load_workflow, named_step

#: Fragments the installer script must contain, with the reason for each.
REQUIRED_FRAGMENTS = (
    (
        'expected_mdtablefix_version="mdtablefix ${MDTABLEFIX_VERSION}"',
        "pin the expected version",
    ),
    ("mdtablefix --version", "inspect the installed version"),
    ("tr -d '\\r'", "normalise Windows version output"),
    (
        '[[ "${installed_mdtablefix_version}" != "${expected_mdtablefix_version}" ]]',
        "replace a missing or mismatched formatter",
    ),
    (
        "cargo binstall --no-confirm --locked --disable-strategies compile",
        (
            "reject a source build, because cargo-binstall's default "
            "strategies end in `compile`"
        ),
    ),
)

#: Fragments whose presence would mean the job compiles the formatter.
FORBIDDEN_FRAGMENTS = (("cargo install", "compile mdtablefix from source"),)


def test_mdtablefix_installers_require_the_pinned_version() -> None:
    """Both formatter installers replace stale executables and verify the pin."""
    for workflow_path, job_name in SETUP_RUST_JOBS:
        step = named_step(
            job_steps(load_workflow(workflow_path), job_name),
            "Install mdtablefix",
        )
        match step.get("run"):
            case str() as run:
                missing = [
                    reason
                    for fragment, reason in REQUIRED_FRAGMENTS
                    if fragment not in run
                ]
                assert not missing, f"{job_name} must {'; '.join(missing)}"
                present = [
                    reason
                    for fragment, reason in FORBIDDEN_FRAGMENTS
                    if fragment in run
                ]
                assert not present, f"{job_name} must not {'; '.join(present)}"
            case _:
                pytest.fail(f"{job_name} must configure mdtablefix")
