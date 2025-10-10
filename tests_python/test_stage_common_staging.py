"""Behavioural tests for staging artefact handling."""

from __future__ import annotations

import importlib
import json
import os
from pathlib import Path, PurePosixPath, PureWindowsPath

import pytest

from stage_test_helpers import decode_output_file, write_workspace_inputs


def test_stage_artefacts_exports_metadata(
    stage_common: object, workspace: Path
) -> None:
    """The staging pipeline should copy inputs, hash them, and export outputs."""

    target = "x86_64-unknown-linux-gnu"
    write_workspace_inputs(workspace, target)

    config = stage_common.StagingConfig(
        workspace=workspace,
        bin_name="netsuke",
        dist_dir="dist",
        checksum_algorithm="sha256",
        artefacts=[
            stage_common.ArtefactConfig(
                source="target/{target}/release/{bin_name}{bin_ext}",
                required=True,
                output="binary_path",
            ),
            stage_common.ArtefactConfig(
                source="target/generated-man/{target}/release/{bin_name}.1",
                required=True,
                output="man_path",
            ),
            stage_common.ArtefactConfig(
                source="LICENSE",
                required=True,
                output="license_path",
            ),
        ],
        platform="linux",
        arch="amd64",
        target=target,
    )

    github_output = workspace / "outputs.txt"
    result = stage_common.stage_artefacts(config, github_output)

    staging_dir = workspace / "dist" / "netsuke_linux_amd64"
    assert result.staging_dir == staging_dir, "StageResult must record the staging directory"
    assert staging_dir.exists(), "Expected staging directory to be created"

    staged_files = {path.name for path in result.staged_artefacts}
    assert staged_files == {"netsuke", "netsuke.1", "LICENSE"}, "Unexpected artefacts staged"
    assert set(result.outputs) == {"binary_path", "man_path", "license_path"}, "Outputs missing expected keys"
    expected_checksums = {
        "netsuke": staging_dir / "netsuke.sha256",
        "netsuke.1": staging_dir / "netsuke.1.sha256",
        "LICENSE": staging_dir / "LICENSE.sha256",
    }
    assert set(result.checksums) == set(expected_checksums), "Checksum outputs missing entries"
    for path in expected_checksums.values():
        assert path.exists(), f"Checksum file {path.name} was not written"

    outputs = decode_output_file(github_output)
    assert outputs["artifact_dir"] == staging_dir.as_posix(), "artifact_dir output should reference staging directory"
    assert outputs["binary_path"].endswith("netsuke"), "binary_path output should point to the staged executable"
    assert outputs["license_path"].endswith("LICENSE"), "license_path output should point to the staged licence"
    artefact_map = json.loads(outputs["artefact_map"])
    assert artefact_map["binary_path"].endswith("netsuke"), "artefact map should include the binary path"
    checksum_map = json.loads(outputs["checksum_map"])
    assert set(checksum_map) == {"netsuke", "netsuke.1", "LICENSE"}, "Checksum map missing entries"


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
    for idx in range(3):
        candidate = build_dir / f"{idx}" / "out" / "netsuke.1"
        candidate.parent.mkdir(parents=True, exist_ok=True)
        candidate.write_text(f".TH {idx}", encoding="utf-8")
        os.utime(candidate, (100 + idx, 100 + idx))

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


def test_match_candidate_path_handles_windows_drive(
    stage_common: object, workspace: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Absolute Windows-style globs should resolve relative to the drive root."""

    monkeypatch.chdir(workspace)

    drive_root = Path("C:\\")
    windows_workspace = drive_root / "workspace"
    man_dir = windows_workspace / "man"
    man_dir.mkdir(parents=True, exist_ok=True)
    candidate = man_dir / "netsuke.1"
    candidate.write_text(".TH WINDOWS", encoding="utf-8")

    staging = importlib.import_module("stage_common.staging")
    matched = staging._match_candidate_path(
        windows_workspace, "C:/workspace/man/*.1"
    )

    assert matched == candidate


def test_stage_artefacts_warns_for_optional(
    stage_common: object, workspace: Path, capfd: pytest.CaptureFixture[str]
) -> None:
    """Optional artefacts should emit a warning when absent but not abort."""

    target = "x86_64-unknown-linux-gnu"
    write_workspace_inputs(workspace, target)

    config = stage_common.StagingConfig(
        workspace=workspace,
        bin_name="netsuke",
        dist_dir="dist",
        checksum_algorithm="sha256",
        artefacts=[
            stage_common.ArtefactConfig(
                source="target/{target}/release/{bin_name}{bin_ext}",
                required=True,
            ),
            stage_common.ArtefactConfig(
                source="missing.txt",
                required=False,
                output="missing",
            ),
        ],
        platform="linux",
        arch="amd64",
        target=target,
    )

    stage_common.stage_artefacts(config, workspace / "out.txt")
    captured = capfd.readouterr()
    assert (
        "::warning title=Artefact Skipped::Optional artefact missing" in captured.err
    ), "Optional artefact warning missing"


def test_stage_artefacts_rejects_reserved_outputs(
    stage_common: object, workspace: Path
) -> None:
    """Custom outputs must not collide with the reserved output keys."""

    target = "x86_64-unknown-linux-gnu"
    write_workspace_inputs(workspace, target)

    config = stage_common.StagingConfig(
        workspace=workspace,
        bin_name="netsuke",
        dist_dir="dist",
        checksum_algorithm="sha256",
        artefacts=[
            stage_common.ArtefactConfig(
                source="target/{target}/release/{bin_name}{bin_ext}",
                required=True,
                output="artifact_dir",
            ),
        ],
        platform="linux",
        arch="amd64",
        target=target,
    )

    with pytest.raises(stage_common.StageError) as exc:
        stage_common.stage_artefacts(config, workspace / "outputs.txt")

    message = str(exc.value)
    assert "collide" in message
    assert "artifact_dir" in message


def test_stage_artefacts_rejects_duplicate_destinations(
    stage_common: object, workspace: Path
) -> None:
    """Staging should fail when two artefacts render the same destination."""

    target = "x86_64-unknown-linux-gnu"
    write_workspace_inputs(workspace, target)

    config = stage_common.StagingConfig(
        workspace=workspace,
        bin_name="netsuke",
        dist_dir="dist",
        checksum_algorithm="sha256",
        artefacts=[
            stage_common.ArtefactConfig(
                source="target/{target}/release/{bin_name}{bin_ext}",
                required=True,
                destination="netsuke",
            ),
            stage_common.ArtefactConfig(
                source="LICENSE",
                required=True,
                destination="netsuke",
            ),
        ],
        platform="linux",
        arch="amd64",
        target=target,
    )

    with pytest.raises(stage_common.StageError, match="Duplicate staged destination"):
        stage_common.stage_artefacts(config, workspace / "outputs.txt")


def test_stage_artefacts_rejects_duplicate_outputs(
    stage_common: object, workspace: Path
) -> None:
    """Staging should fail when two artefacts export the same output key."""

    target = "x86_64-unknown-linux-gnu"
    write_workspace_inputs(workspace, target)

    config = stage_common.StagingConfig(
        workspace=workspace,
        bin_name="netsuke",
        dist_dir="dist",
        checksum_algorithm="sha256",
        artefacts=[
            stage_common.ArtefactConfig(
                source="target/{target}/release/{bin_name}{bin_ext}",
                required=True,
                output="binary_path",
            ),
            stage_common.ArtefactConfig(
                source="LICENSE",
                required=True,
                output="binary_path",
            ),
        ],
        platform="linux",
        arch="amd64",
        target=target,
    )

    with pytest.raises(stage_common.StageError, match="Duplicate artefact output key"):
        stage_common.stage_artefacts(config, workspace / "outputs.txt")


def test_stage_artefacts_fails_with_attempt_context(
    stage_common: object, workspace: Path
) -> None:
    """Missing required artefacts should include context in the error message."""

    config = stage_common.StagingConfig(
        workspace=workspace,
        bin_name="netsuke",
        dist_dir="dist",
        checksum_algorithm="sha256",
        artefacts=[
            stage_common.ArtefactConfig(
                source="missing-{target}",
                required=True,
            ),
        ],
        platform="linux",
        arch="amd64",
        target="x86_64-unknown-linux-gnu",
    )

    with pytest.raises(stage_common.StageError) as exc:
        stage_common.stage_artefacts(config, workspace / "outputs.txt")

    message = str(exc.value)
    assert "Workspace=" in message, "Workspace context missing from error"
    assert "missing-{target}" in message, "Template pattern missing from error"
    assert (
        "missing-x86_64-unknown-linux-gnu" in message
    ), "Rendered path missing from error"


def test_glob_root_and_pattern_handles_windows_drive(stage_common: object) -> None:
    """Absolute Windows globs should strip the drive before globbing."""

    staging = importlib.import_module("stage_common.staging")
    helper = staging._glob_root_and_pattern

    root, pattern = helper(PureWindowsPath("C:/dist/*.zip"))
    assert root == "C:\\"
    assert pattern == "dist/*.zip"


def test_glob_root_and_pattern_returns_wildcard_for_root_only(  # noqa: ARG001
    stage_common: object,
) -> None:
    """Root-only absolute paths should glob all children."""

    # Fixture import triggers plugin registration; the value itself is unused.
    staging = importlib.import_module("stage_common.staging")
    helper = staging._glob_root_and_pattern

    root, pattern = helper(PureWindowsPath("C:/"))
    assert root == "C:\\"
    assert pattern == "*"


def test_glob_root_and_pattern_handles_posix_absolute(  # noqa: ARG001
    stage_common: object,
) -> None:
    """POSIX absolute paths should preserve relative segments for globbing."""

    # Fixture import triggers plugin registration; the value itself is unused.
    staging = importlib.import_module("stage_common.staging")
    helper = staging._glob_root_and_pattern

    root, pattern = helper(PurePosixPath("/tmp/dist/*.zip"))
    assert root == "/"
    assert pattern.endswith("dist/*.zip"), pattern


def test_glob_root_and_pattern_rejects_relative_paths(  # noqa: ARG001
    stage_common: object,
) -> None:
    """Relative globs should be rejected to avoid ambiguous anchors."""

    # Fixture import triggers plugin registration; the value itself is unused.
    staging = importlib.import_module("stage_common.staging")
    helper = staging._glob_root_and_pattern

    with pytest.raises(ValueError, match="Expected absolute path"):
        helper(PurePosixPath("dist/*.zip"))


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
