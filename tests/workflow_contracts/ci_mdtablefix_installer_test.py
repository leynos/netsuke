"""Hold the Markdown formatter installer's version and source-build contract.

`make check-fmt` shells out to `mdtablefix`, so both formatter jobs install a
pinned release before running it. The crate's binstall metadata is broken
(`bin-dir = "."`, leynos/mdtablefix#458), so the installer no longer goes
through `cargo binstall`: Linux takes the published tarball against a pinned
SHA-256, and Windows, for which no binary is published at all, compiles once
per cache generation into a directory that never shares compiler output with
the product.

Run via ``make test-workflow-contracts``.
"""

import yaml
from workflow_loading import (
    REPO_ROOT,
    SETUP_RUST_JOBS,
    job_steps,
    load_workflow,
    named_step,
    require_mapping,
)

INSTALL_ACTION = "./.github/actions/install-mdtablefix"
ACTION_PATH = REPO_ROOT / ".github" / "actions" / "install-mdtablefix" / "action.yml"

#: Fragments the installer script must contain, with the reason for each.
REQUIRED_FRAGMENTS = (
    ('expected="mdtablefix ${MDTABLEFIX_VERSION}"', "pin the expected version"),
    ("mdtablefix --version", "inspect the installed version"),
    ("tr -d '\\r'", "normalise Windows version output"),
    ('if [[ "${installed}" == "${expected}" ]]', "reuse a cached executable"),
    (
        '"${MDTABLEFIX_SHA256}" "${tarball}" | sha256sum --check --',
        "verify the downloaded tarball against the pinned digest by name",
    ),
    (
        'tar --extract --gzip --file "${tarball}"',
        "extract only the executable from the verified tarball",
    ),
    (
        'CARGO_TARGET_DIR="${MDTABLEFIX_BUILD_DIR}"',
        "keep the Windows source build out of the product's target directory",
    ),
    ("leynos/mdtablefix#458", "cite the removal condition for the workaround"),
)


def test_both_formatter_jobs_use_the_pinned_installer_action() -> None:
    """Both formatter jobs install mdtablefix through the shared action."""
    for workflow_path, job_name in SETUP_RUST_JOBS:
        step = named_step(
            job_steps(load_workflow(workflow_path), job_name), "Install mdtablefix"
        )
        assert step.get("uses") == INSTALL_ACTION, (
            f"{job_name} must install mdtablefix through {INSTALL_ACTION}, "
            f"got {step.get('uses')!r}"
        )
        inputs = require_mapping(step.get("with"), f"{job_name} installer inputs")
        assert inputs.get("build-dir"), (
            f"{job_name} must give the source build a dedicated target directory"
        )


def test_the_installer_verifies_its_download_and_bounds_its_fallback() -> None:
    """The installer pins a digest and never builds into the product's target."""
    document = require_mapping(
        yaml.safe_load(ACTION_PATH.read_text(encoding="utf-8")), "install-mdtablefix"
    )
    runs = require_mapping(document.get("runs"), "runs")
    steps = runs.get("steps")
    assert isinstance(steps, list), "the action must declare a step list"
    assert steps, "the action must declare at least one step"
    step = require_mapping(steps[0], "the installer step")
    script = str(step.get("run", ""))
    missing = [
        reason for fragment, reason in REQUIRED_FRAGMENTS if fragment not in script
    ]
    assert not missing, f"the installer must {'; '.join(missing)}"
    env = require_mapping(step.get("env"), "the installer env")
    digest = str(env.get("MDTABLEFIX_SHA256", ""))
    assert len(digest) == 64, (
        f"the installer must pin a full SHA-256 digest, got {digest!r}"
    )
    assert all(character in "0123456789abcdef" for character in digest), (
        f"the pinned digest must be lowercase hexadecimal, got {digest!r}"
    )
    assert "target/" not in script, (
        "the source build must not write into the product's target directory"
    )
