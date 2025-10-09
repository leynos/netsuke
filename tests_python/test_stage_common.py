"""Tests for the shared staging helpers used by release workflows.

Summary
-------
Validate that ``stage_common.stage_artifacts`` bundles the licence file
alongside binaries and manual pages so GitHub Actions outputs include the
expected ``license_path`` entry.

Purpose
-------
Regression tests for the Windows release job that previously failed when
``license_path`` was missing from the recorded outputs. Ensures staging
fails fast if the licence is absent and records the correct path when
present.

Usage
-----
Run the tests directly::

    python -m pytest tests_python/test_stage_common.py

Examples
--------
Create a dummy workspace with a binary, manual page, and licence::

    from pathlib import Path
    from stage_common import StagingConfig, stage_artifacts

    workspace = Path("/tmp/workspace")
    workspace.mkdir(parents=True, exist_ok=True)
    (workspace / "LICENSE").write_text("Example", encoding="utf-8")
    binary = workspace / "target" / "x" / "release" / "netsuke"
    binary.parent.mkdir(parents=True, exist_ok=True)
    binary.write_bytes(b"binary")
    man = workspace / "target" / "generated-man" / "x" / "release" / "netsuke.1"
    man.parent.mkdir(parents=True, exist_ok=True)
    man.write_text(".TH", encoding="utf-8")

    config = StagingConfig(
        bin_name="netsuke",
        target="x",
        platform="linux",
        arch="amd64",
        workspace=workspace,
    )
    stage_artifacts(config, workspace / "out.txt")
"""

from __future__ import annotations

import importlib.util
import sys
import typing as typ
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parent.parent
MODULE_PATH = REPO_ROOT / ".github" / "workflows" / "scripts" / "stage_common.py"


if typ.TYPE_CHECKING:
    from types import ModuleType
else:
    ModuleType = type(sys)


@pytest.fixture(scope="session")
def stage_common() -> ModuleType:
    """Load the ``stage_common`` helper once for reuse across tests."""

    spec = importlib.util.spec_from_file_location("stage_common", MODULE_PATH)
    if spec is None or spec.loader is None:
        message = "Failed to load stage_common module"
        raise RuntimeError(message)

    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def _prepare_workspace(root: Path, *, bin_ext: str = "") -> tuple[str, str]:
    """Create a fake cargo workspace with binary, man page, and licence."""

    bin_name = "netsuke"
    target = "x86_64-pc-windows-msvc"

    licence = root / "LICENSE"
    licence.write_text("Copyright Netsuke", encoding="utf-8")

    bin_path = root / "target" / target / "release" / f"{bin_name}{bin_ext}"
    bin_path.parent.mkdir(parents=True, exist_ok=True)
    bin_path.write_bytes(b"binary")

    man_path = (
        root
        / "target"
        / "generated-man"
        / target
        / "release"
        / f"{bin_name}.1"
    )
    man_path.parent.mkdir(parents=True, exist_ok=True)
    man_path.write_text(".TH NETSUKE 1", encoding="utf-8")

    return bin_name, target


def test_stage_artifacts_records_license(stage_common: ModuleType, tmp_path: Path) -> None:
    """The staged bundle should include and reference the licence file."""

    workspace = tmp_path / "workspace"
    workspace.mkdir()
    bin_name, target = _prepare_workspace(workspace, bin_ext=".exe")

    config = stage_common.StagingConfig(
        bin_name=bin_name,
        target=target,
        platform="windows",
        arch="amd64",
        workspace=workspace,
        bin_ext=".exe",
    )
    github_output = workspace / "outputs.txt"

    result = stage_common.stage_artifacts(config, github_output)

    expected_dir = workspace / "dist" / config.artifact_dir_name
    expected_license = expected_dir / "LICENSE"

    assert result.artifact_dir == expected_dir
    assert result.binary_path.name.endswith(".exe")
    assert result.man_path.name.endswith(".1")
    assert result.license_path == expected_license
    assert expected_license.read_text(encoding="utf-8") == "Copyright Netsuke"
    license_checksum = Path(f"{expected_license}.sha256")
    assert license_checksum.exists()
    assert "LICENSE" in license_checksum.read_text(encoding="utf-8")

    outputs = github_output.read_text(encoding="utf-8").splitlines()
    output_map = dict(line.split("=", 1) for line in outputs if line)
    assert output_map["artifact_dir"] == expected_dir.as_posix()
    assert output_map["binary_path"] == result.binary_path.as_posix()
    assert output_map["man_path"] == result.man_path.as_posix()
    assert output_map["license_path"] == expected_license.as_posix()


def test_stage_artifacts_requires_license(stage_common: ModuleType, tmp_path: Path) -> None:
    """It should surface a descriptive error when the licence is missing."""

    workspace = tmp_path / "workspace"
    workspace.mkdir()
    bin_name, target = _prepare_workspace(workspace)
    (workspace / "LICENSE").unlink()

    config = stage_common.StagingConfig(
        bin_name=bin_name,
        target=target,
        platform="linux",
        arch="amd64",
        workspace=workspace,
    )

    with pytest.raises(RuntimeError, match="Licence file not found"):
        stage_common.stage_artifacts(config, workspace / "outputs.txt")


def test_validate_and_locate_sources_success(
    stage_common: ModuleType, tmp_path: Path
) -> None:
    """Helper should resolve the binary, manual, and licence paths."""

    workspace = tmp_path / "workspace"
    workspace.mkdir()
    bin_name, target = _prepare_workspace(workspace)

    bin_src, man_src, licence_src = stage_common._validate_and_locate_sources(
        workspace, target, bin_name, ""
    )

    assert bin_src.name == bin_name
    assert man_src.name == f"{bin_name}.1"
    assert licence_src.name == "LICENSE"


def test_validate_and_locate_sources_requires_binary(
    stage_common: ModuleType, tmp_path: Path
) -> None:
    """Helper should fail fast when the compiled binary is missing."""

    workspace = tmp_path / "workspace"
    workspace.mkdir()
    bin_name, target = _prepare_workspace(workspace)
    (workspace / "target" / target / "release" / bin_name).unlink()

    with pytest.raises(RuntimeError, match="Binary not found"):
        stage_common._validate_and_locate_sources(workspace, target, bin_name, "")


def test_prepare_artifact_directory_cleans_previous(
    stage_common: ModuleType, tmp_path: Path
) -> None:
    """Existing artefacts should be cleared so stale files never leak."""

    dist_dir = tmp_path / "workspace" / "dist"
    artifact_dir = dist_dir / "bundle"
    artifact_dir.mkdir(parents=True, exist_ok=True)
    stale = artifact_dir / "old.txt"
    stale.write_text("stale", encoding="utf-8")

    result = stage_common._prepare_artifact_directory(dist_dir, "bundle")

    assert result == artifact_dir
    assert result.is_dir()
    assert not stale.exists()


def test_write_github_outputs_overwrites_existing_content(
    stage_common: ModuleType, tmp_path: Path
) -> None:
    """Outputs file should be rewritten each run to avoid duplicate keys."""

    workspace = tmp_path / "workspace"
    artifact_dir = workspace / "dist" / "bundle"
    artifact_dir.mkdir(parents=True, exist_ok=True)
    bin_path = artifact_dir / "netsuke"
    man_path = artifact_dir / "netsuke.1"
    licence_path = artifact_dir / "LICENSE"
    for file in (bin_path, man_path, licence_path):
        file.write_text(file.name, encoding="utf-8")

    github_output = workspace / "outputs.txt"
    github_output.write_text("stale", encoding="utf-8")

    stage_common._write_github_outputs(
        github_output,
        stage_common.StageResult(
            artifact_dir=artifact_dir,
            binary_path=bin_path,
            man_path=man_path,
            license_path=licence_path,
        ),
    )

    contents = github_output.read_text(encoding="utf-8").splitlines()
    keys = [line.split("=", 1)[0] for line in contents if line]
    assert keys == ["artifact_dir", "binary_path", "man_path", "license_path"]
    assert all("stale" not in line for line in contents)


def test_write_github_outputs_errors_on_empty_value(
    stage_common: ModuleType, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Guard against empty outputs by surfacing a descriptive error."""

    workspace = tmp_path / "workspace"
    artifact_dir = workspace / "dist"
    artifact_dir.mkdir(parents=True, exist_ok=True)
    github_output = workspace / "outputs.txt"

    def _always_empty(_: Path) -> str:
        return ""

    monkeypatch.setattr(stage_common, "_escape_output_value", _always_empty)

    with pytest.raises(RuntimeError, match="unexpectedly empty"):
        stage_common._write_github_outputs(
            github_output,
            stage_common.StageResult(
                artifact_dir=artifact_dir,
                binary_path=artifact_dir / "bin",
                man_path=artifact_dir / "man",
                license_path=artifact_dir / "license",
            ),
        )
