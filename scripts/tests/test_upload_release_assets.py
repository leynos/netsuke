"""Tests for the upload_release_assets helper script."""

from __future__ import annotations

import importlib.util
import subprocess
import sys
from pathlib import Path

import pytest


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT_PATH = REPO_ROOT / "scripts" / "upload_release_assets.py"


@pytest.fixture(scope="session")
def module():
    spec = importlib.util.spec_from_file_location(
        "upload_release_assets", SCRIPT_PATH
    )
    module = importlib.util.module_from_spec(spec)  # type: ignore[arg-type]
    assert spec and spec.loader
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)  # type: ignore[assignment]
    return module


def create_file(path: Path, content: bytes = b"data") -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(content)


def test_discover_assets_collects_expected_files(module, tmp_path: Path) -> None:
    dist = tmp_path / "dist"
    create_file(dist / "linux" / "netsuke", b"binary")
    create_file(dist / "linux" / "netsuke.sha256", b"checksum")
    create_file(dist / "linux" / "netsuke.deb", b"deb")
    create_file(dist / "windows" / "netsuke.exe", b"exe")
    create_file(dist / "windows" / "netsuke.msi", b"msi")
    create_file(dist / "man" / "netsuke.1", b"man")

    assets = module.discover_assets(dist, bin_name="netsuke")

    assert [asset.asset_name for asset in assets] == [
        "linux-netsuke",
        "netsuke.deb",
        "linux-netsuke.sha256",
        "man-netsuke.1",
        "windows-netsuke.exe",
        "windows-netsuke.msi",
    ]


def test_discover_assets_rejects_duplicates(module, tmp_path: Path) -> None:
    dist = tmp_path / "dist"
    create_file(dist / "a" / "netsuke.pkg", b"pkg-a")
    create_file(dist / "b" / "netsuke.pkg", b"pkg-b")

    with pytest.raises(module.AssetError) as exc:
        module.discover_assets(dist, bin_name="netsuke")

    assert "Asset name collision" in str(exc.value)


def test_discover_assets_rejects_empty_files(module, tmp_path: Path) -> None:
    dist = tmp_path / "dist"
    create_file(dist / "linux" / "netsuke", b"")

    with pytest.raises(module.AssetError) as exc:
        module.discover_assets(dist, bin_name="netsuke")

    assert "is empty" in str(exc.value)


def test_cli_dry_run_outputs_summary(module, tmp_path: Path) -> None:
    dist = tmp_path / "dist"
    create_file(dist / "linux" / "netsuke", b"binary")
    create_file(dist / "linux" / "netsuke.sha256", b"checksum")

    result = subprocess.run(
        [
            sys.executable,
            str(SCRIPT_PATH),
            "--release-tag",
            "v1.2.3",
            "--bin-name",
            "netsuke",
            "--dist-dir",
            str(dist),
            "--dry-run",
        ],
        capture_output=True,
        text=True,
        check=False,
    )

    assert result.returncode == 0
    assert "Planned uploads:" in result.stdout
    assert "linux-netsuke" in result.stdout
    assert "linux-netsuke.sha256" in result.stdout
    assert "[dry-run] gh release upload v1.2.3" in result.stdout
