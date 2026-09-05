"""Hold the Markdown formatter installer to the shared prebuilt-only action.

`make check-fmt` shells out to `mdtablefix`, so both formatter jobs install a
pinned release before running it. Both now use the shared `install-mdtablefix`
action, which installs from a published archive and never compiles.

This repository carried its own action until mdtablefix 0.5.1. Its binstall
metadata was broken (`bin-dir = "."`, leynos/mdtablefix#458) and no Windows
archive existed at all (leynos/mdtablefix#459), so the local action took the
Linux tarball against a pinned SHA-256 and compiled the tool on Windows under a
documented exception. 0.5.1 publishes archives for both platforms, so the
exception, the local action and its Windows build directory are all gone.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
from cache_contract_data import WORKFLOW_DIR
from workflow_loading import (
    REPO_ROOT,
    SETUP_RUST_JOBS,
    job_steps,
    load_workflow,
    named_step,
    require_mapping,
)

if typ.TYPE_CHECKING:  # pragma: no cover - imported for annotations only
    import pathlib as pl

#: The shared action, which is the only installer either lane may use.
SHARED_ACTION = "leynos/shared-actions/.github/actions/install-mdtablefix@"

#: The earliest release that publishes an archive for every platform this
#: repository formats on. Pinning below it would reintroduce a compile on
#: Windows, which is the thing this contract exists to prevent.
MINIMUM_VERSION = (0, 5, 1)

#: The local action this replaced. Its absence is asserted rather than assumed,
#: because a half-finished revert would leave both installers present and the
#: lanes would keep working while the exception quietly returned.
RETIRED_ACTION_DIR = REPO_ROOT / ".github" / "actions" / "install-mdtablefix"


def _version_tuple(value: str) -> tuple[int, ...]:
    """Return a dotted version as integers for comparison."""
    return tuple(int(part) for part in value.split("."))


@pytest.mark.parametrize(("workflow_path", "job_name"), SETUP_RUST_JOBS)
def test_both_formatter_jobs_use_the_shared_installer_action(
    workflow_path: pl.Path, job_name: str
) -> None:
    """Require every formatter lane to install through the shared action.

    Both lanes, not just Linux. Windows is the one that used to compile, so a
    contract that checked only the lane which already installed a binary would
    have passed throughout the period this change exists to end.
    """
    step = named_step(
        job_steps(load_workflow(workflow_path), job_name), "Install mdtablefix"
    )
    uses = str(step.get("uses", ""))

    assert uses.startswith(SHARED_ACTION), (
        f"{job_name} must install mdtablefix through the shared action, got {uses!r}"
    )
    assert len(uses.rsplit("@", 1)[1]) == 40, (
        f"{job_name} must pin the shared action to a full commit SHA, got {uses!r}"
    )


def _resolve_literal(version: str, workflow_path: pl.Path) -> str:
    """Return the literal version a lane pins, following any indirection.

    Both lanes pass the version through `${{ env.MDTABLEFIX_VERSION }}`, and
    the Windows lane lives in a reusable workflow whose own `env` takes the
    value from a workflow input. The literal therefore sits in `ci.yml`, one
    or two hops away, and a contract that stopped at the first expression
    would assert nothing about either lane.

    Parameters
    ----------
    version
        The value the installer step passes, literal or expression.
    workflow_path
        The workflow the step belongs to.

    Returns
    -------
    str
        The dotted version literal.
    """
    if not version.startswith("${{"):
        return version
    # Each hop is checked before it is followed. Resolving any expression
    # through `MDTABLEFIX_VERSION` would let the step switch to an unrelated
    # variable while this contract kept reading the old, still-correct pin and
    # reporting success for a lane receiving something else.
    assert "env.MDTABLEFIX_VERSION" in version, (
        f"the installer step should take its version from MDTABLEFIX_VERSION, "
        f"got {version!r}"
    )
    workflow = load_workflow(workflow_path)
    env = require_mapping(workflow.get("env"), f"{workflow_path.name} env")
    resolved = str(env.get("MDTABLEFIX_VERSION", ""))
    assert resolved, f"{workflow_path.name} must declare MDTABLEFIX_VERSION"
    if not resolved.startswith("${{"):
        return resolved
    assert "mdtablefix-version" in resolved, (
        f"{workflow_path.name} should take MDTABLEFIX_VERSION from its own "
        f"mdtablefix-version input, got {resolved!r}"
    )
    # A reusable workflow's env takes the value from its caller's `with:`.
    jobs = require_mapping(
        load_workflow(WORKFLOW_DIR / "ci.yml").get("jobs"), "ci.yml jobs"
    )
    callers = []
    for name, declaration in jobs.items():
        job = require_mapping(declaration, f"ci.yml {name}")
        if not str(job.get("uses", "")).endswith(workflow_path.name):
            continue
        supplied = require_mapping(job.get("with"), f"{name} with")
        callers.append(str(supplied["mdtablefix-version"]))
    assert len(callers) == 1, (
        f"{workflow_path.name} should have exactly one caller supplying "
        f"mdtablefix-version, found {callers!r}"
    )
    return callers[0]


@pytest.mark.parametrize(("workflow_path", "job_name"), SETUP_RUST_JOBS)
def test_both_formatter_jobs_pin_a_version_that_publishes_archives(
    workflow_path: pl.Path, job_name: str
) -> None:
    """Require a pinned version no earlier than the first with full coverage.

    The shared action refuses anything earlier, so this would surface at run
    time anyway. Holding it here names the reason instead, and fails in the
    contract suite rather than a quarter of an hour into a Windows job.
    """
    step = named_step(
        job_steps(load_workflow(workflow_path), job_name), "Install mdtablefix"
    )
    inputs = require_mapping(step.get("with"), f"{job_name} installer inputs")
    version = str(inputs.get("version", ""))

    assert version, f"{job_name} must pin an mdtablefix version, got {inputs!r}"
    version = _resolve_literal(version, workflow_path)
    assert _version_tuple(version) >= MINIMUM_VERSION, (
        f"{job_name} pins mdtablefix {version}, which predates the first "
        "release publishing an archive for every platform this repository "
        "formats on; an earlier pin reintroduces a Windows source build"
    )


def test_the_local_installer_action_is_gone() -> None:
    """Require the retired local action to be absent, not merely unused.

    Leaving it in place would keep a working source build one `uses:` edit
    away, and its Windows branch would still be the only documented way this
    repository compiles a tool.
    """
    assert not RETIRED_ACTION_DIR.exists(), (
        f"{RETIRED_ACTION_DIR} should have been removed with the exception it "
        "existed to implement"
    )


def test_no_workflow_keeps_the_windows_build_directory() -> None:
    """Require the Windows source build's target directory to be gone.

    It existed only to keep that compile's output away from the product's
    tree. With no compile it has no purpose, and a cache entry still listing
    it would archive an empty path on every Windows run.
    """
    searched = sorted({
        *(REPO_ROOT / ".github" / "workflows").glob("*.yml"),
        *(REPO_ROOT / ".github" / "actions").rglob("action.yml"),
    })
    offenders = [
        path.relative_to(REPO_ROOT).as_posix()
        for path in searched
        if "mdtablefix-build" in path.read_text(encoding="utf-8")
    ]

    assert not offenders, (
        f"{offenders!r} still reference the retired Windows build directory"
    )
