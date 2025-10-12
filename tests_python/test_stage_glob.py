"""Tests validating glob resolution and cross-platform path handling."""

from __future__ import annotations

import os
from pathlib import Path

import pytest

from stage_test_helpers import write_workspace_inputs


def test_stage_artefacts_uses_alternative_glob(
    stage_common: object, workspace: Path
) -> None:
    """Fallback paths should be used when the preferred template is absent."""

    target = "x86_64-unknown-linux-gnu"
    write_workspace_inputs(workspace, target)
    generated = (
        workspace / "target" / "generated-man" / target / "release" / "netsuke.1"
    )
    generated.unlink()

    build_dir = workspace / "target" / target / "release" / "build"
    first = build_dir / "1" / "out" / "netsuke.1"
    second = build_dir / "2" / "out" / "netsuke.1"
    first.parent.mkdir(parents=True, exist_ok=True)
    second.parent.mkdir(parents=True, exist_ok=True)
    first.write_text(".TH 1", encoding="utf-8")
    second.write_text(".TH 2", encoding="utf-8")
    os.utime(first, (first.stat().st_atime, first.stat().st_mtime - 100))

    config = stage_common.StagingConfig(
        workspace=workspace,
        bin_name="netsuke",
        dist_dir="dist",
        checksum_algorithm="sha256",
        artefacts=[
            stage_common.ArtefactConfig(
                source="target/generated-man/{target}/release/{bin_name}.1",
                required=True,
                output="man_path",
                alternatives=["target/{target}/release/build/*/out/{bin_name}.1"],
            ),
        ],
        platform="linux",
        arch="amd64",
        target=target,
    )

    github_output = workspace / "outputs.txt"
    result = stage_common.stage_artefacts(config, github_output)
    staged_path = result.outputs["man_path"]
    assert (
        staged_path.read_text(encoding="utf-8") == ".TH 2"
    ), "Fallback glob should pick the newest man page"


def test_stage_artefacts_glob_selects_newest_candidate(
    stage_common: object, workspace: Path
) -> None:
    """Glob matches should resolve to the most recently modified file."""

    target = "x86_64-unknown-linux-gnu"
    write_workspace_inputs(workspace, target)
    generated = (
        workspace / "target" / "generated-man" / target / "release" / "netsuke.1"
    )
    generated.unlink()

    build_dir = workspace / "target" / target / "release" / "build"
    build_dir.mkdir(parents=True, exist_ok=True)
    candidates = []
    for idx in range(3):
        candidate = build_dir / f"{idx}" / "out" / "netsuke.1"
        candidate.parent.mkdir(parents=True, exist_ok=True)
        candidate.write_text(f".TH {idx}", encoding="utf-8")
        os.utime(candidate, (100 + idx, 100 + idx))
        candidates.append(candidate)

    config = stage_common.StagingConfig(
        workspace=workspace,
        bin_name="netsuke",
        dist_dir="dist",
        checksum_algorithm="sha256",
        artefacts=[
            stage_common.ArtefactConfig(
                source="target/generated-man/{target}/release/{bin_name}.1",
                required=True,
                output="man_path",
                alternatives=["target/{target}/release/build/*/out/{bin_name}.1"],
            ),
        ],
        platform="linux",
        arch="amd64",
        target=target,
    )

    github_output = workspace / "outputs.txt"
    result = stage_common.stage_artefacts(config, github_output)
    staged_path = result.outputs["man_path"]
    assert (
        staged_path.read_text(encoding="utf-8") == ".TH 2"
    ), "Glob resolution should select the most recent candidate"
    latest = max(candidates, key=lambda f: f.stat().st_mtime_ns)
    assert (
        latest.read_text(encoding="utf-8") == staged_path.read_text(encoding="utf-8")
    ), "Selected candidate should match the most recent file"


def test_stage_artefacts_matches_absolute_glob(
    stage_common: object, workspace: Path
) -> None:
    """Absolute glob patterns should allow staging artefacts."""

    absolute_root = workspace / "absolute" / "1.2.3"
    absolute_root.mkdir(parents=True, exist_ok=True)
    source_path = absolute_root / "netsuke.txt"
    source_path.write_text("payload", encoding="utf-8")

    artefact = stage_common.ArtefactConfig(
        source=f"{workspace.as_posix()}/absolute/*/netsuke.txt",
        required=True,
        output="absolute_path",
    )
    config = stage_common.StagingConfig(
        workspace=workspace,
        bin_name="netsuke",
        dist_dir="dist",
        checksum_algorithm="sha256",
        artefacts=[artefact],
        platform="linux",
        arch="amd64",
        target="x86_64-unknown-linux-gnu",
    )

    github_output = workspace / "github_output.txt"
    result = stage_common.stage_artefacts(config, github_output)

    assert result.staged_artefacts, "Expected artefact to be staged from absolute glob"
    staged_path = result.staged_artefacts[0]
    assert staged_path.read_text(encoding="utf-8") == "payload"
    assert result.outputs["absolute_path"] == staged_path


@pytest.mark.parametrize(
    ("pattern", "description"),
    [
        ("C:/workspace/man/*.1", "glob pattern"),
        ("C:/workspace/man/netsuke.1", "absolute path"),
    ],
)
def test_match_candidate_path_handles_windows_paths(
    staging_resolution: object,
    workspace: Path,
    monkeypatch: pytest.MonkeyPatch,
    pattern: str,
    description: str,
) -> None:
    """Absolute Windows-style paths should resolve correctly.

    Tests both glob patterns and direct file paths.
    """

    monkeypatch.chdir(workspace)
    drive_root = Path("C:\\")
    windows_workspace = drive_root / "workspace"
    man_dir = windows_workspace / "man"
    man_dir.mkdir(parents=True, exist_ok=True)
    candidate = man_dir / "netsuke.1"
    candidate.write_text(".TH WINDOWS", encoding="utf-8")

    matched = staging_resolution._match_candidate_path(windows_workspace, pattern)

    assert matched == candidate, f"Expected Windows {description} to resolve"


def test_match_candidate_path_prefers_newest_relative_glob(
    staging_resolution: object, tmp_path: Path
) -> None:
    """Relative glob resolution should select the most recent candidate."""

    dist_dir = tmp_path / "dist"
    dist_dir.mkdir()
    older = dist_dir / "old.txt"
    newer = dist_dir / "new.txt"
    older.write_text("old", encoding="utf-8")
    newer.write_text("new", encoding="utf-8")
    os.utime(older, (100, 100))
    os.utime(newer, (200, 200))

    matched = staging_resolution._match_candidate_path(tmp_path, "dist/*.txt")

    assert matched == newer


def test_match_candidate_path_handles_posix_absolute_glob(
    staging_resolution: object, tmp_path: Path
) -> None:
    """Absolute POSIX-style globs should match files on disk."""

    dist_dir = tmp_path / "dist"
    dist_dir.mkdir()
    candidate = dist_dir / "netsuke.txt"
    candidate.write_text("payload", encoding="utf-8")

    pattern = f"{dist_dir.as_posix()}/*.txt"
    matched = staging_resolution._match_candidate_path(tmp_path, pattern)

    assert matched == candidate


def test_match_candidate_path_returns_none_when_missing(
    staging_resolution: object, tmp_path: Path
) -> None:
    """Unmatched paths should return ``None`` rather than raising errors."""

    assert (
        staging_resolution._match_candidate_path(tmp_path, "missing.txt") is None
    )
