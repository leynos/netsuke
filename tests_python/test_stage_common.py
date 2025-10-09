"""Tests for the reusable staging helper module.

Summary
-------
Validate that the TOML loader merges configuration blocks correctly and that
``stage_artefacts`` stages binaries, manuals, and licences while publishing
useful metadata for later workflow steps.

Usage
-----
Run the tests directly::

    uvx --with "cyclopts>=0.14" pytest tests_python/test_stage_common.py
"""

from __future__ import annotations

import importlib.util
import json
import os
import sys
import typing as typ
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parent.parent
MODULE_PATH = (
    REPO_ROOT / ".github" / "actions" / "stage" / "scripts" / "stage_common.py"
)

if typ.TYPE_CHECKING:
    from types import ModuleType
else:
    ModuleType = type(sys)


@pytest.fixture(scope="session")
def stage_common() -> ModuleType:
    """Load the staging helper once for reuse across tests."""
    spec = importlib.util.spec_from_file_location("stage_common", MODULE_PATH)
    if spec is None or spec.loader is None:
        message = "Failed to load stage_common module"
        raise RuntimeError(message)

    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


@pytest.fixture
def workspace(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    """Create an isolated workspace and set ``GITHUB_WORKSPACE`` accordingly."""
    root = tmp_path / "workspace"
    root.mkdir()
    monkeypatch.setenv("GITHUB_WORKSPACE", str(root))
    return root


def _decode_output_file(path: Path) -> dict[str, str]:
    """Parse the key-value pairs written to ``GITHUB_OUTPUT``."""
    lines = [line for line in path.read_text(encoding="utf-8").splitlines() if line]
    values: dict[str, str] = {}
    for line in lines:
        key, value = line.split("=", 1)
        decoded = value.replace("%0A", "\n").replace("%0D", "\r").replace("%25", "%")
        values[key] = decoded
    return values


def _write_workspace_inputs(root: Path, target: str) -> None:
    bin_path = root / "target" / target / "release" / "netsuke"
    bin_path.parent.mkdir(parents=True, exist_ok=True)
    bin_path.write_bytes(b"binary")

    man_path = root / "target" / "generated-man" / target / "release" / "netsuke.1"
    man_path.parent.mkdir(parents=True, exist_ok=True)
    man_path.write_text(".TH NETSUKE 1", encoding="utf-8")

    licence = root / "LICENSE"
    licence.write_text("Copyright Netsuke", encoding="utf-8")


def test_load_config_merges_common_and_target(
    stage_common: ModuleType, workspace: Path
) -> None:
    """``load_config`` should merge common values with the requested target."""
    config_file = workspace / "release-staging.toml"
    config_file.write_text(
        "\n".join(
            [
                "[common]",
                'bin_name = "netsuke"',
                'dist_dir = "dist"',
                'checksum_algorithm = "sha256"',
                "artefacts = [",
                (
                    '  { source = "target/{target}/release/{bin_name}{bin_ext}",'
                    ' required = true, output = "binary_path" },'
                ),
                '  { source = "LICENSE", required = true, output = "license_path" },',
                "]",
                "",
                "[targets.test]",
                'platform = "linux"',
                'arch = "amd64"',
                'target = "x86_64-unknown-linux-gnu"',
            ]
        ),
        encoding="utf-8",
    )

    config = stage_common.load_config(config_file, "test")

    assert config.workspace == workspace
    assert config.bin_name == "netsuke"
    assert config.platform == "linux"
    assert config.arch == "amd64"
    assert config.target == "x86_64-unknown-linux-gnu"
    assert config.checksum_algorithm == "sha256"
    assert [item.output for item in config.artefacts] == ["binary_path", "license_path"]


def test_stage_artefacts_exports_metadata(
    stage_common: ModuleType, workspace: Path
) -> None:
    """The staging pipeline should copy inputs, hash them, and export outputs."""
    target = "x86_64-unknown-linux-gnu"
    _write_workspace_inputs(workspace, target)

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
    assert result.staging_dir == staging_dir
    assert staging_dir.exists()

    staged_files = {path.name for path in result.staged_artefacts}
    assert staged_files == {"netsuke", "netsuke.1", "LICENSE"}
    assert set(result.outputs) == {"binary_path", "man_path", "license_path"}
    expected_checksums = {
        "netsuke": staging_dir / "netsuke.sha256",
        "netsuke.1": staging_dir / "netsuke.1.sha256",
        "LICENSE": staging_dir / "LICENSE.sha256",
    }
    assert set(result.checksums) == set(expected_checksums)
    for path in expected_checksums.values():
        assert path.exists()

    outputs = _decode_output_file(github_output)
    assert outputs["artifact_dir"] == staging_dir.as_posix()
    artefact_map = json.loads(outputs["artefact_map"])
    assert artefact_map["binary_path"].endswith("netsuke")
    checksum_map = json.loads(outputs["checksum_map"])
    assert set(checksum_map) == {"netsuke", "netsuke.1", "LICENSE"}


def test_stage_artefacts_uses_alternative_glob(
    stage_common: ModuleType, workspace: Path
) -> None:
    """Fallback paths should be used when the preferred template is absent."""
    target = "x86_64-unknown-linux-gnu"
    _write_workspace_inputs(workspace, target)
    # Remove the generated man page so only the glob match remains.
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
    assert staged_path.read_text(encoding="utf-8") == ".TH 2"


def test_stage_artefacts_warns_for_optional(
    stage_common: ModuleType, workspace: Path, capfd: pytest.CaptureFixture[str]
) -> None:
    """Optional artefacts should emit a warning when absent but not abort."""
    target = "x86_64-unknown-linux-gnu"
    _write_workspace_inputs(workspace, target)

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
    assert "::warning title=Artefact Skipped::Optional artefact missing" in captured.err
