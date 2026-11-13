"""Refuse a damaged staged tree before the declared-asset action reaches GitHub.

Every way the downloaded tree can disagree with the release is driven here
through the action's real shell bodies: a record from another commit or run, a
configured target with no record or two, a release-shaped file nobody declared,
one asset staged twice, bytes replaced after their producer recorded them, and
a declared package that is a link rather than the bytes themselves.

The property each case holds is the same one, and it is the one that matters:
the refusal happens at selection, names its own diagnosis, and no ``gh`` call
is made at all. That last assertion is what distinguishes a refusal from a
reconciliation that merely decided not to upload -- a run that reads the
release has already told GitHub the release exists.

The tree is built by ``upload_declared_assets_staging.py`` and executed by
``upload_declared_assets_harness.py``; the undamaged cases live in
``upload_declared_assets_execution_test.py``.

Run via ``make test-workflow-contracts``.
"""

import shutil
import typing as typ

import pytest
from upload_declared_assets_harness import execute
from upload_declared_assets_staging import (
    DIST,
    archive_name,
    package_names,
    record_path,
    release_facts,
    rewrite_record,
    stage_workspace,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    from pathlib import Path

    from upload_declared_assets_staging import Facts


def _stale_commit(workspace: Path, facts: Facts) -> None:
    """Claim the records were built from another commit."""
    rewrite_record(workspace, facts.targets[0], commit="b" * 40)


def _foreign_run(workspace: Path, facts: Facts) -> None:
    """Claim the records were written by another workflow run."""
    rewrite_record(workspace, facts.targets[0], run_id="4243")


def _missing_target(workspace: Path, facts: Facts) -> None:
    """Remove one target's producer record entirely."""
    record_path(workspace, facts.targets[0]).unlink()


def _duplicate_target(workspace: Path, facts: Facts) -> None:
    """Make two records claim the same configured target."""
    shutil.copy2(
        record_path(workspace, facts.targets[0]),
        record_path(workspace, facts.targets[1]),
    )


def _release_shaped_stray(workspace: Path, facts: Facts) -> None:
    """Stage an archive-shaped file no producer declares."""
    stray = workspace / DIST / f"stray-{facts.version}.tar.gz"
    stray.write_bytes(b"stray\n")


def _ambiguous_asset(workspace: Path, facts: Facts) -> None:
    """Publish one archive name from two places in the tree."""
    target = facts.targets[0]
    archive = archive_name(facts, target)
    shutil.copy2(workspace / DIST / archive, workspace / DIST / target.key / archive)


def _replaced_bytes(workspace: Path, facts: Facts) -> None:
    """Replace a staged package's bytes after its producer recorded them."""
    target = facts.targets[0]
    path = workspace / DIST / target.key / package_names(facts, target)[0]
    path.write_bytes(b"rebuilt elsewhere\n")


def _symlinked_asset(workspace: Path, facts: Facts) -> None:
    """Stage one declared package as a link rather than the bytes themselves."""
    target = facts.targets[0]
    path = workspace / DIST / target.key / package_names(facts, target)[0]
    path.unlink()
    path.symlink_to(workspace / DIST / archive_name(facts, target))


#: Each way a staged tree can disagree with the release, and the diagnosis the
#: operator must be given for it. Every entry must stop at selection: the tree
#: is refused before the uploader can read the release at all.
DAMAGE: dict[str, tuple[cabc.Callable[[Path, Facts], None], str]] = {
    "ambiguous-asset": (_ambiguous_asset, "was staged 2 times"),
    "duplicate-target": (
        _duplicate_target,
        "more than one producer record covers targets",
    ),
    "foreign-run": (_foreign_run, "not the current run's"),
    "missing-target": (_missing_target, "no producer record covers targets"),
    "release-shaped-stray": (
        _release_shaped_stray,
        "is release-shaped but not declared",
    ),
    "replaced-bytes": (_replaced_bytes, "holds bytes its producer did not record"),
    "stale-candidate": (_stale_commit, "not candidate"),
    "symlinked-asset": (_symlinked_asset, "was not staged"),
}


@pytest.mark.parametrize("kind", sorted(DAMAGE))
def test_a_refused_tree_stops_before_any_network_call(
    tmp_path: Path, kind: str
) -> None:
    """Stop at selection, naming the fault, before anything is uploaded."""
    facts = release_facts()
    workspace = stage_workspace(tmp_path, facts)
    damage, diagnosis = DAMAGE[kind]
    damage(workspace, facts)
    run = execute(tmp_path, workspace, facts)
    assert len(run.completed) == 1, "selection must be the failing step"
    assert run.completed[0].returncode == 1, run.completed[0].stderr
    assert diagnosis in run.completed[0].stderr, run.completed[0].stderr
    assert not run.gh_log, "a refused tree must not reach the network"
