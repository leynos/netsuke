"""Behavioural and wiring tests for the cargo-binstall hoist step.

``scripts/hoist_binstall_archives.py`` moves the staged
``{package}-{version}-{target}.tar.gz`` archives (and their ``.sha256``
sidecars) from the downloaded workflow-artefact layout to the ``dist/`` root,
where ``upload-release-assets`` publishes them under the plain names the
binstall metadata in ``Cargo.toml`` resolves. These tests stage representative
layouts and pin the load-bearing behaviour: every expected target must be
present with its checksum before anything moves, and a shortfall fails with
the missing names listed rather than uploading a partial asset set.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import sys
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "scripts"))

import hoist_binstall_archives as hoist_mod  # noqa: E402

WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "release.yml"

STAGING_CONFIG = """\
[common]
bin_name = "netsuke"

[common.binstall]
enabled = true

[targets.linux-x86_64]
target = "x86_64-unknown-linux-gnu"

[targets.macos-aarch64]
target = "aarch64-apple-darwin"
"""

MANIFEST = """\
[package]
name = "netsuke-build"
version = "9.9.9"
"""

VERSION = "9.9.9"
EXPECTED_NAMES = [
    "netsuke-build-9.9.9-aarch64-apple-darwin.tar.gz",
    "netsuke-build-9.9.9-x86_64-unknown-linux-gnu.tar.gz",
]


@pytest.fixture
def workspace(tmp_path: Path) -> dict[str, Path]:
    """Provide a staging config, manifest, and empty dist directory."""
    staging = tmp_path / "release-staging.toml"
    staging.write_text(STAGING_CONFIG, encoding="utf-8")
    manifest = tmp_path / "Cargo.toml"
    manifest.write_text(MANIFEST, encoding="utf-8")
    dist = tmp_path / "dist"
    dist.mkdir()
    return {"staging": staging, "manifest": manifest, "dist": dist}


def stage_pair(dist: Path, artefact: str, staging_dir: str, name: str) -> None:
    """Create an archive and its sidecar in the nested artefact layout."""
    nested = dist / artefact / staging_dir
    nested.mkdir(parents=True, exist_ok=True)
    (nested / name).write_bytes(b"archive")
    (nested / f"{name}.sha256").write_text("checksum", encoding="utf-8")


def run_hoist(workspace: dict[str, Path]) -> int:
    """Invoke the hoist against the fixture workspace."""
    return hoist_mod.hoist(
        workspace["dist"], workspace["staging"], workspace["manifest"], VERSION
    )


def test_expected_names_derive_from_staging_and_manifest(
    workspace: dict[str, Path],
) -> None:
    """The expected archive set comes from the config files, not literals."""
    names = hoist_mod.expected_archive_names(
        workspace["staging"], workspace["manifest"], VERSION
    )
    assert names == EXPECTED_NAMES


def test_hoist_moves_every_archive_and_sidecar_to_the_root(
    workspace: dict[str, Path],
) -> None:
    """The happy path relocates both files of every pair to dist/."""
    stage_pair(workspace["dist"], "netsuke-linux-amd64", "s1", EXPECTED_NAMES[1])
    stage_pair(workspace["dist"], "netsuke-macos-arm64", "s2", EXPECTED_NAMES[0])

    assert run_hoist(workspace) == 0
    for name in EXPECTED_NAMES:
        assert (workspace["dist"] / name).is_file()
        assert (workspace["dist"] / f"{name}.sha256").is_file()


def test_hoist_fails_listing_missing_targets_without_moving(
    workspace: dict[str, Path],
    capsys: pytest.CaptureFixture[str],
) -> None:
    """A missing target aborts before any file moves and names the gap."""
    stage_pair(workspace["dist"], "netsuke-linux-amd64", "s1", EXPECTED_NAMES[1])

    assert run_hoist(workspace) == 1
    captured = capsys.readouterr()
    assert EXPECTED_NAMES[0] in captured.err
    assert not (workspace["dist"] / EXPECTED_NAMES[1]).exists()
    nested = workspace["dist"] / "netsuke-linux-amd64" / "s1" / EXPECTED_NAMES[1]
    assert nested.is_file()


def test_hoist_fails_when_a_checksum_sidecar_is_absent(
    workspace: dict[str, Path],
    capsys: pytest.CaptureFixture[str],
) -> None:
    """An archive without its .sha256 sidecar fails validation."""
    stage_pair(workspace["dist"], "netsuke-linux-amd64", "s1", EXPECTED_NAMES[1])
    stage_pair(workspace["dist"], "netsuke-macos-arm64", "s2", EXPECTED_NAMES[0])
    (
        workspace["dist"]
        / "netsuke-macos-arm64"
        / "s2"
        / f"{EXPECTED_NAMES[0]}.sha256"
    ).unlink()

    assert run_hoist(workspace) == 1
    captured = capsys.readouterr()
    assert f"{EXPECTED_NAMES[0]}.sha256" in captured.err


def test_hoist_fails_on_an_empty_dist(
    workspace: dict[str, Path],
    capsys: pytest.CaptureFixture[str],
) -> None:
    """No staged archives at all is a hard failure, not a no-op."""
    assert run_hoist(workspace) == 1
    captured = capsys.readouterr()
    for name in EXPECTED_NAMES:
        assert name in captured.err


def test_hoist_rejects_duplicate_archive_candidates(
    workspace: dict[str, Path],
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Two nested copies of one archive are ambiguous and fail validation."""
    stage_pair(workspace["dist"], "netsuke-linux-amd64", "s1", EXPECTED_NAMES[1])
    stage_pair(workspace["dist"], "netsuke-linux-amd64-copy", "s1", EXPECTED_NAMES[1])
    stage_pair(workspace["dist"], "netsuke-macos-arm64", "s2", EXPECTED_NAMES[0])

    assert run_hoist(workspace) == 1
    captured = capsys.readouterr()
    assert "found 2 candidates" in captured.err


def test_release_workflow_invokes_the_hoist_script() -> None:
    """The release job must run the script with the resolved version."""
    workflow = WORKFLOW_PATH.read_text(encoding="utf-8")
    assert "scripts/hoist_binstall_archives.py" in workflow
    assert "--version '${{ needs.metadata.outputs.version }}'" in workflow
