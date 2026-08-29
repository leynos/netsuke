"""Rollback-transaction tests for the cargo-binstall hoist step.

The move phase is all-or-nothing: a mid-phase failure must restore every
completed move so the release job can rerun after the fault clears, and a
failed restoration must surface both errors rather than losing the cause.
These tests inject faults at the ``shutil.move`` seam to prove both
properties. Shared fixtures and helpers live in ``conftest.py``.

Run via ``make test-workflow-contracts``.
"""

import itertools
import shutil
import typing as typ
from unittest import mock

import hoist_binstall_archives as hoist_mod
import pytest
from conftest import EXPECTED_NAMES, run_hoist, stage_pair

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    from pathlib import Path

#: The forward move that aborts the transaction. Two moves relocate the first
#: archive/sidecar pair, so the third call is the first move of the second
#: pair — by which point there is something to roll back.
ABORTING_MOVE_CALL = 3


def _move_failing_at_the_third_call(
    *, fail_rollback: bool
) -> cabc.Callable[[Path, Path], str | Path]:
    """Return a ``shutil.move`` stand-in that aborts the forward move phase."""
    real_move = shutil.move
    call_numbers = itertools.count(1)

    def failing_move(src: Path, dst: Path) -> str | Path:
        call = next(call_numbers)
        if call == ABORTING_MOVE_CALL:
            msg = "injected move failure"
            raise OSError(msg)
        if fail_rollback and call > ABORTING_MOVE_CALL:
            msg = "injected rollback failure"
            raise OSError(msg)
        return real_move(src, dst)

    return failing_move


def _stage_both_pairs(dist: Path) -> dict[str, Path]:
    """Stage both expected pairs, returning each name's nested directory."""
    stage_pair(dist, "netsuke-linux-amd64/s1", EXPECTED_NAMES[1])
    stage_pair(dist, "netsuke-macos-arm64/s2", EXPECTED_NAMES[0])
    return {
        EXPECTED_NAMES[0]: dist / "netsuke-macos-arm64/s2",
        EXPECTED_NAMES[1]: dist / "netsuke-linux-amd64/s1",
    }


def _assert_restored(dist: Path, name: str, nested: Path) -> None:
    """Assert ``name`` left the release root and returned to ``nested`` intact."""
    assert not (dist / name).exists(), (
        f"{name} must not remain at the release root after rollback"
    )
    assert not (dist / f"{name}.sha256").exists(), (
        f"{name}.sha256 must not remain at the release root after rollback"
    )
    restored = nested / name
    assert restored.is_file(), f"{name} must return to its nested path"
    assert restored.read_bytes() == f"archive:{name}".encode(), (
        f"{name} must keep its original content through the rollback"
    )
    sidecar = nested / f"{name}.sha256"
    assert sidecar.is_file(), f"{name}.sha256 must return to its nested path"
    assert sidecar.read_text(encoding="utf-8") == f"checksum:{name}", (
        f"{name}.sha256 must keep its original content through the rollback"
    )


def test_hoist_rolls_back_completed_moves_when_a_move_fails(
    workspace: dict[str, Path],
) -> None:
    """A mid-phase move failure restores every file, and a rerun succeeds."""
    sources = _stage_both_pairs(workspace["dist"])

    # The patch is scoped to the failing run so the retry below exercises the
    # real `shutil.move`, proving the rollback left a rerunnable tree.
    with (
        mock.patch.object(
            hoist_mod.shutil,
            "move",
            _move_failing_at_the_third_call(fail_rollback=False),
        ),
        pytest.raises(OSError, match="injected move failure"),
    ):
        run_hoist(workspace)

    for name, nested in sources.items():
        _assert_restored(workspace["dist"], name, nested)

    assert run_hoist(workspace) == 0, "a rerun after the fault clears must succeed"
    for name in EXPECTED_NAMES:
        assert (workspace["dist"] / name).is_file(), (
            f"{name} must reach the release root on the retry"
        )
        assert (workspace["dist"] / name).read_bytes() == (
            f"archive:{name}".encode()
        ), f"{name} content must survive the retry"


def test_hoist_surfaces_both_errors_when_the_rollback_also_fails(
    workspace: dict[str, Path],
) -> None:
    """A failed rollback reports its own error alongside the original.

    Losing the move failure to the rollback failure would leave an operator
    with no way to tell why the release aborted, so both must survive.
    """
    _stage_both_pairs(workspace["dist"])

    with (
        mock.patch.object(
            hoist_mod.shutil,
            "move",
            _move_failing_at_the_third_call(fail_rollback=True),
        ),
        pytest.raises(BaseExceptionGroup) as raised,
    ):
        run_hoist(workspace)

    group = raised.value
    assert "rollback could not restore every file" in str(group), (
        f"the group must explain that the rollback failed; got {group}"
    )
    assert sorted(str(error) for error in group.exceptions) == [
        "injected move failure",
        "injected rollback failure",
    ], f"both failures must survive in the group; got {group.exceptions}"
    assert (workspace["dist"] / EXPECTED_NAMES[0]).is_file(), (
        "an unrestorable pair must be left where it landed, not silently "
        "discarded, so the operator can inspect the release root"
    )
