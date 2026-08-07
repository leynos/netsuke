"""Example-based behavioural tests for the cargo-binstall hoist step.

These tests stage representative layouts and pin the load-bearing
behaviour: every expected target must be present with its checksum before
anything moves, every validation failure leaves the tree untouched
(all-or-none), and a shortfall fails with the missing names listed rather
than uploading a partial asset set. Shared fixtures and helpers live in
``conftest.py``; the rollback-transaction suite lives in
``hoist_binstall_rollback_test.py``, the generated-layout suite in
``hoist_binstall_archives_generated_test.py``, and the ``release.yml``
wiring contracts in ``release_workflow_hoist_test.py``.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import itertools
import os
import typing as typ

# conftest.py inserts scripts/ onto sys.path before this module is imported.
import hoist_binstall_archives as hoist_mod
import pytest
from conftest import (
    EXPECTED_NAMES,
    VERSION,
    assert_nothing_moved,
    run_hoist,
    stage_pair,
)

if typ.TYPE_CHECKING:
    from pathlib import Path


def test_expected_names_derive_from_staging_and_manifest(
    workspace: dict[str, Path],
) -> None:
    """The expected archive set comes from the config files, not literals."""
    names = hoist_mod.expected_archive_names(
        workspace["staging"], workspace["manifest"], VERSION
    )
    assert names == EXPECTED_NAMES, (
        f"expected names should derive from staging config and manifest; got {names}"
    )


def test_expected_names_track_alternative_inputs(tmp_path: Path) -> None:
    """Different package names, versions, and targets flow into the names."""
    staging = tmp_path / "staging.toml"
    staging.write_text(
        '[targets.only]\ntarget = "riscv64gc-unknown-linux-gnu"\n',
        encoding="utf-8",
    )
    manifest = tmp_path / "Cargo.toml"
    manifest.write_text('[package]\nname = "otherpkg"\n', encoding="utf-8")

    names = hoist_mod.expected_archive_names(staging, manifest, "3.2.1")
    assert names == ["otherpkg-3.2.1-riscv64gc-unknown-linux-gnu.tar.gz"], (
        f"names must interpolate package, version, and target; got {names}"
    )


@pytest.mark.parametrize(
    "staging_content",
    ["[targets]\n", ""],
    ids=["empty-table", "missing-table"],
)
def test_expected_names_reject_an_empty_target_table(
    tmp_path: Path, staging_content: str
) -> None:
    """A staging config without targets is a configuration error.

    An omitted ``[targets]`` table reports the same named contract failure
    as an empty one, rather than a bare ``KeyError``.
    """
    staging = tmp_path / "staging.toml"
    staging.write_text(staging_content, encoding="utf-8")
    manifest = tmp_path / "Cargo.toml"
    manifest.write_text('[package]\nname = "pkg"\n', encoding="utf-8")

    with pytest.raises(ValueError, match="defines no release targets"):
        hoist_mod.expected_archive_names(staging, manifest, "1.0.0")


def test_expected_names_reject_duplicate_target_triples(tmp_path: Path) -> None:
    """Two entries sharing a triple are rejected before anything moves.

    A repeated triple would derive the same archive name twice, so the same
    staged file would be claimed and moved twice; the second move would fail
    on the already-relocated source and roll back an otherwise valid release.
    """
    staging = tmp_path / "staging.toml"
    staging.write_text(
        '[targets.linux]\ntarget = "x86_64-unknown-linux-gnu"\n'
        '[targets.linux-again]\ntarget = "x86_64-unknown-linux-gnu"\n',
        encoding="utf-8",
    )
    manifest = tmp_path / "Cargo.toml"
    manifest.write_text('[package]\nname = "pkg"\n', encoding="utf-8")

    with pytest.raises(ValueError, match="duplicate release targets"):
        hoist_mod.expected_archive_names(staging, manifest, "1.0.0")


def test_hoist_moves_every_archive_and_sidecar_to_the_root(
    workspace: dict[str, Path],
) -> None:
    """The happy path relocates both files of every pair, preserving content."""
    stage_pair(workspace["dist"], "netsuke-linux-amd64/s1", EXPECTED_NAMES[1])
    stage_pair(workspace["dist"], "netsuke-macos-arm64/s2", EXPECTED_NAMES[0])

    assert run_hoist(workspace) == 0, "hoist should succeed when all pairs staged"
    for name in EXPECTED_NAMES:
        moved = workspace["dist"] / name
        assert moved.is_file(), f"{name} should sit at the dist root after hoisting"
        assert moved.read_bytes() == f"archive:{name}".encode(), (
            f"{name} content must survive the move"
        )
        sidecar = workspace["dist"] / f"{name}.sha256"
        assert sidecar.is_file(), f"{name}.sha256 should sit at the dist root"
        assert sidecar.read_text(encoding="utf-8") == f"checksum:{name}", (
            f"{name}.sha256 content must survive the move"
        )
    leftovers = [
        path
        for path in workspace["dist"].rglob("*.tar.gz*")
        if path.parent != workspace["dist"]
    ]
    assert leftovers == [], f"no staged copies should remain nested: {leftovers}"


@pytest.mark.parametrize(
    ("first_state", "second_state"),
    [
        pair
        for pair in itertools.product(
            ["ok", "missing-archive", "missing-sidecar"], repeat=2
        )
        if pair != ("ok", "ok")
    ],
)
def test_hoist_moves_nothing_for_every_failing_state_combination(
    workspace: dict[str, Path],
    capsys: pytest.CaptureFixture[str],
    first_state: str,
    second_state: str,
) -> None:
    """Exhaustively: any target in a failing state means nothing moves."""
    for name, state, artefact in (
        (EXPECTED_NAMES[0], first_state, "a1"),
        (EXPECTED_NAMES[1], second_state, "a2"),
    ):
        if state == "missing-archive":
            continue
        stage_pair(
            workspace["dist"],
            f"{artefact}/s",
            name,
            with_sidecar=state != "missing-sidecar",
        )

    assert run_hoist(workspace) == 1, (
        f"states ({first_state}, {second_state}) must fail validation"
    )
    assert_nothing_moved(workspace["dist"], EXPECTED_NAMES)
    captured = capsys.readouterr()
    assert "Missing cargo-binstall release assets" in captured.err, (
        "the failure must list the missing assets on stderr"
    )


def test_hoist_failure_names_the_missing_target(
    workspace: dict[str, Path],
    capsys: pytest.CaptureFixture[str],
) -> None:
    """A missing target's archive name appears verbatim in the error."""
    stage_pair(workspace["dist"], "netsuke-linux-amd64/s1", EXPECTED_NAMES[1])

    assert run_hoist(workspace) == 1, "a missing target must fail the hoist"
    captured = capsys.readouterr()
    assert EXPECTED_NAMES[0] in captured.err, (
        f"stderr must name the missing archive {EXPECTED_NAMES[0]}"
    )
    nested = workspace["dist"] / "netsuke-linux-amd64" / "s1" / EXPECTED_NAMES[1]
    assert nested.is_file(), "the staged archive must stay in place on failure"


def test_hoist_fails_when_a_checksum_sidecar_is_absent(
    workspace: dict[str, Path],
    capsys: pytest.CaptureFixture[str],
) -> None:
    """An archive without its .sha256 sidecar fails validation untouched."""
    stage_pair(workspace["dist"], "netsuke-linux-amd64/s1", EXPECTED_NAMES[1])
    stage_pair(
        workspace["dist"],
        "netsuke-macos-arm64/s2",
        EXPECTED_NAMES[0],
        with_sidecar=False,
    )

    assert run_hoist(workspace) == 1, "a missing sidecar must fail the hoist"
    captured = capsys.readouterr()
    assert f"{EXPECTED_NAMES[0]}.sha256" in captured.err, (
        "stderr must name the absent checksum sidecar"
    )
    assert_nothing_moved(workspace["dist"], EXPECTED_NAMES)


def test_hoist_rejects_duplicate_archive_candidates(
    workspace: dict[str, Path],
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Two nested copies of one archive are ambiguous; nothing moves."""
    stage_pair(workspace["dist"], "netsuke-linux-amd64/s1", EXPECTED_NAMES[1])
    stage_pair(workspace["dist"], "netsuke-linux-amd64-copy/s1", EXPECTED_NAMES[1])
    stage_pair(workspace["dist"], "netsuke-macos-arm64/s2", EXPECTED_NAMES[0])

    assert run_hoist(workspace) == 1, "duplicate candidates must fail the hoist"
    captured = capsys.readouterr()
    assert "found 2 candidates" in captured.err, (
        "stderr must report the ambiguous candidate count"
    )
    assert_nothing_moved(workspace["dist"], EXPECTED_NAMES)


def test_hoist_rejects_an_occupied_destination(
    workspace: dict[str, Path],
    capsys: pytest.CaptureFixture[str],
) -> None:
    """A same-named file already at the dist root blocks the move."""
    stage_pair(workspace["dist"], "netsuke-linux-amd64/s1", EXPECTED_NAMES[1])
    stage_pair(workspace["dist"], "netsuke-macos-arm64/s2", EXPECTED_NAMES[0])
    (workspace["dist"] / EXPECTED_NAMES[0]).write_bytes(b"impostor")

    assert run_hoist(workspace) == 1, "a destination collision must fail the hoist"
    captured = capsys.readouterr()
    assert "destination already occupied" in captured.err, (
        "stderr must report the destination collision"
    )
    assert (workspace["dist"] / EXPECTED_NAMES[0]).read_bytes() == b"impostor", (
        "the pre-existing root file must not be overwritten"
    )
    staged = workspace["dist"] / "netsuke-linux-amd64" / "s1" / EXPECTED_NAMES[1]
    assert staged.is_file(), "staged archives must stay in place on collision"


@pytest.mark.parametrize("colliding_suffix", ["", ".sha256"])
def test_hoist_rejects_a_directory_at_the_destination(
    workspace: dict[str, Path],
    capsys: pytest.CaptureFixture[str],
    colliding_suffix: str,
) -> None:
    """A directory occupying either destination is a collision, not a target."""
    stage_pair(workspace["dist"], "netsuke-linux-amd64/s1", EXPECTED_NAMES[1])
    stage_pair(workspace["dist"], "netsuke-macos-arm64/s2", EXPECTED_NAMES[0])
    blocking = workspace["dist"] / f"{EXPECTED_NAMES[0]}{colliding_suffix}"
    blocking.mkdir()

    assert run_hoist(workspace) == 1, (
        f"a directory at {blocking.name} must fail validation"
    )
    captured = capsys.readouterr()
    assert "destination already occupied" in captured.err, (
        "stderr must report the directory collision"
    )
    assert blocking.is_dir(), "the blocking directory must be left untouched"
    for nested_dir, name in (
        ("netsuke-linux-amd64/s1", EXPECTED_NAMES[1]),
        ("netsuke-macos-arm64/s2", EXPECTED_NAMES[0]),
    ):
        staged = workspace["dist"] / nested_dir / name
        assert staged.is_file(), (
            f"staged archive {name} must stay in place on a collision"
        )
    assert not (workspace["dist"] / EXPECTED_NAMES[1]).exists(), (
        "no valid pair may move while any destination is occupied"
    )


@pytest.mark.parametrize("colliding_suffix", ["", ".sha256"])
def test_hoist_rejects_a_dangling_symlink_at_the_destination(
    workspace: dict[str, Path],
    capsys: pytest.CaptureFixture[str],
    colliding_suffix: str,
) -> None:
    """A dangling symlink occupying either destination is still a collision.

    ``exists()`` would report the dangling link as absent and let the move
    silently follow it; the ``lstat`` probe must treat any entry, including a
    link to nowhere, as occupying the destination.
    """
    stage_pair(workspace["dist"], "netsuke-linux-amd64/s1", EXPECTED_NAMES[1])
    stage_pair(workspace["dist"], "netsuke-macos-arm64/s2", EXPECTED_NAMES[0])
    blocking = workspace["dist"] / f"{EXPECTED_NAMES[0]}{colliding_suffix}"
    blocking.symlink_to(workspace["dist"] / "points-at-nothing")

    assert run_hoist(workspace) == 1, (
        f"a dangling symlink at {blocking.name} must fail validation"
    )
    captured = capsys.readouterr()
    assert "destination already occupied" in captured.err, (
        "stderr must report the dangling-symlink collision"
    )
    assert blocking.is_symlink(), "the blocking symlink must be left untouched"
    assert_nothing_moved(workspace["dist"], EXPECTED_NAMES)


@pytest.mark.parametrize("linked_suffix", ["", ".sha256"])
def test_hoist_rejects_a_symlinked_staged_asset(
    workspace: dict[str, Path],
    capsys: pytest.CaptureFixture[str],
    linked_suffix: str,
) -> None:
    """A symlink standing in for either staged file fails validation.

    Moving a symlink would publish a broken link rather than the archive it
    points at, so both halves of the pair must be regular files.
    """
    stage_pair(workspace["dist"], "netsuke-linux-amd64/s1", EXPECTED_NAMES[1])
    nested = workspace["dist"] / "netsuke-macos-arm64/s2"
    nested.mkdir(parents=True)
    real = workspace["dist"] / "real-payload"
    real.write_bytes(b"payload")
    linked_name = f"{EXPECTED_NAMES[0]}{linked_suffix}"
    (nested / linked_name).symlink_to(real)
    if linked_suffix:
        (nested / EXPECTED_NAMES[0]).write_bytes(b"archive")
    else:
        (nested / f"{EXPECTED_NAMES[0]}.sha256").write_text("sum", encoding="utf-8")

    assert run_hoist(workspace) == 1, f"a symlink at {linked_name} must fail validation"
    captured = capsys.readouterr()
    expected = "checksum sidecar absent" if linked_suffix else "not a regular file"
    assert expected in captured.err, (
        f"stderr must reject the symlinked asset; got {captured.err}"
    )
    assert_nothing_moved(workspace["dist"], EXPECTED_NAMES)


@pytest.mark.skipif(os.geteuid() == 0, reason="root bypasses permission bits")
def test_hoist_propagates_traversal_errors(
    workspace: dict[str, Path],
) -> None:
    """An unreadable directory surfaces as an error, not a missing asset."""
    blocked = workspace["dist"] / "blocked"
    blocked.mkdir()
    blocked.chmod(0o000)
    try:
        with pytest.raises(PermissionError):
            run_hoist(workspace)
    finally:
        blocked.chmod(0o755)


def test_main_runs_the_hoist_from_cli_arguments(
    workspace: dict[str, Path],
    capsys: pytest.CaptureFixture[str],
) -> None:
    """The CLI entry point used by release.yml wires arguments to the hoist."""
    stage_pair(workspace["dist"], "netsuke-linux-amd64/s1", EXPECTED_NAMES[1])
    stage_pair(workspace["dist"], "netsuke-macos-arm64/s2", EXPECTED_NAMES[0])

    status = hoist_mod.main([
        "--version",
        VERSION,
        "--dist-dir",
        str(workspace["dist"]),
        "--staging-config",
        str(workspace["staging"]),
        "--manifest",
        str(workspace["manifest"]),
    ])
    assert status == 0, "the CLI must exit 0 on a fully staged dist"
    captured = capsys.readouterr()
    assert "Hoisted 2 archive/sidecar pairs" in captured.out, (
        "the CLI must report the hoisted pair count"
    )
    for name in EXPECTED_NAMES:
        assert (workspace["dist"] / name).is_file(), (
            f"the CLI invocation must move {name} to the dist root"
        )
