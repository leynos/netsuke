"""Tests validating glob resolution and cross-platform path handling."""

from __future__ import annotations

import os
from pathlib import Path, PurePosixPath, PureWindowsPath

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


def test_match_candidate_path_handles_windows_drive(
    staging_module: object, workspace: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Absolute Windows-style globs should resolve relative to the drive root."""
    monkeypatch.chdir(workspace)

    drive_root = Path("C:\\")
    windows_workspace = drive_root / "workspace"
    man_dir = windows_workspace / "man"
    man_dir.mkdir(parents=True, exist_ok=True)
    candidate = man_dir / "netsuke.1"
    candidate.write_text(".TH WINDOWS", encoding="utf-8")

    matched = staging_module._match_candidate_path(
        windows_workspace, "C:/workspace/man/*.1"
    )

    assert matched == candidate


def test_glob_root_and_pattern_handles_windows_drive(staging_module: object) -> None:
    """Absolute Windows globs should strip the drive before globbing."""

    root, pattern = staging_module._glob_root_and_pattern(PureWindowsPath("C:/dist/*.zip"))
    assert root == "C:\\"
    assert pattern == "dist/*.zip"


def test_glob_root_and_pattern_returns_wildcard_for_root_only(
    staging_module: object,
) -> None:
    """Root-only absolute paths should glob all children."""

    root, pattern = staging_module._glob_root_and_pattern(PureWindowsPath("C:/"))
    assert root == "C:\\"
    assert pattern == "*"


def test_glob_root_and_pattern_handles_posix_absolute(staging_module: object) -> None:
    """POSIX absolute paths should preserve relative segments for globbing."""

    root, pattern = staging_module._glob_root_and_pattern(PurePosixPath("/tmp/dist/*.zip"))
    assert root == "/"
    assert pattern.endswith("dist/*.zip"), pattern


def test_glob_root_and_pattern_rejects_relative_paths(staging_module: object) -> None:
    """Relative globs should be rejected to avoid ambiguous anchors."""

    with pytest.raises(ValueError, match="Expected absolute path"):
        staging_module._glob_root_and_pattern(PurePosixPath("dist/*.zip"))


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


def test_match_glob_candidate_handles_absolute_posix(
    staging_module: object, tmp_path: Path
) -> None:
    """Absolute POSIX-style globs should resolve within the workspace."""

    dist_dir = tmp_path / "dist"
    dist_dir.mkdir()
    candidate = dist_dir / "netsuke.txt"
    candidate.write_text("payload", encoding="utf-8")

    pattern = Path(f"{dist_dir.as_posix()}/*.txt")
    matched = staging_module._match_glob_candidate(
        tmp_path, pattern, pattern.as_posix()
    )

    assert matched == candidate


def test_match_glob_candidate_handles_absolute_windows(
    staging_module: object, monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    """Absolute Windows globs should resolve relative to the drive anchor."""

    monkeypatch.chdir(tmp_path)
    drive_root = Path("C:\\")
    workspace = drive_root / "workspace"
    target_dir = workspace / "dist"
    target_dir.mkdir(parents=True, exist_ok=True)
    candidate = target_dir / "netsuke.zip"
    candidate.write_text("payload", encoding="utf-8")

    pattern = "C:/workspace/dist/*.zip"
    matched = staging_module._match_glob_candidate(
        workspace, Path(pattern), pattern
    )

    assert matched == candidate


def test_contains_glob_detects_magic(staging_module: object) -> None:
    """Wildcard detection should align with Python's glob semantics."""

    assert staging_module._contains_glob("*.txt")
    assert staging_module._contains_glob("archive.[0-9]")
    assert not staging_module._contains_glob("plain.txt")


def test_iter_absolute_matches_handles_platforms(
    staging_module: object, monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    """Absolute glob iteration should work for POSIX and simulated Windows paths."""

    # POSIX root
    posix_root = tmp_path / "root"
    target = posix_root / "dist"
    target.mkdir(parents=True)
    posix_file = target / "netsuke.tar.gz"
    posix_file.write_text("payload", encoding="utf-8")
    posix_pattern = PurePosixPath(f"{target.as_posix()}/*.tar.gz")
    posix_matches = list(staging_module._iter_absolute_matches(posix_pattern))
    assert posix_file in posix_matches

    # Windows-style root
    monkeypatch.chdir(tmp_path)
    drive_root = Path("C:\\")
    drive_target = drive_root / "build"
    drive_target.mkdir(parents=True, exist_ok=True)
    windows_file = drive_target / "netsuke.exe"
    windows_file.write_text("payload", encoding="utf-8")
    windows_pattern = PureWindowsPath("C:/build/*.exe")
    windows_matches = list(
        staging_module._iter_absolute_matches(windows_pattern)
    )
    assert windows_file in windows_matches


def test_newest_file_returns_latest_candidate(
    staging_module: object, tmp_path: Path
) -> None:
    """The newest file helper should select the most recently modified path."""

    files = []
    for idx in range(3):
        candidate = tmp_path / f"file{idx}.txt"
        candidate.write_text(str(idx), encoding="utf-8")
        os.utime(candidate, (100 + idx, 100 + idx))
        files.append(candidate)

    newest = staging_module._newest_file(files)
    assert newest == files[-1]
    assert staging_module._newest_file([]) is None
