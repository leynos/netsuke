"""Tests for the upload_release_assets helper script."""

from __future__ import annotations

import importlib.util
import subprocess
import sys
import typing as typ
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT_PATH = REPO_ROOT / "scripts" / "upload_release_assets.py"


if typ.TYPE_CHECKING:
    from types import ModuleType
else:
    ModuleType = type(sys)


@pytest.fixture(scope="session")
def module() -> ModuleType:
    """Load the upload_release_assets script once for reuse across tests."""
    spec = importlib.util.spec_from_file_location("upload_release_assets", SCRIPT_PATH)
    if spec is None:
        message = "Failed to create module spec for upload_release_assets"
        raise RuntimeError(message)
    if spec.loader is None:
        message = "Module spec missing loader for upload_release_assets"
        raise RuntimeError(message)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def create_file(path: Path, content: bytes = b"data") -> None:
    """Create a file with the given content, ensuring parent directories exist."""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(content)


def test_discover_assets_collects_expected_files(
    module: ModuleType, tmp_path: Path
) -> None:
    """Verify that asset discovery preserves expected ordering and names."""
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
        "linux-netsuke.deb",
        "linux-netsuke.sha256",
        "man-netsuke.1",
        "windows-netsuke.exe",
        "windows-netsuke.msi",
    ], "Asset discovery order does not match expected sequence"


def test_discover_assets_disambiguates_duplicate_filenames(
    module: ModuleType, tmp_path: Path
) -> None:
    """It prefixes nested directories to avoid collisions for package files."""
    dist = tmp_path / "dist"
    create_file(dist / "linux" / "pkg" / "netsuke.pkg", b"pkg-linux")
    create_file(dist / "windows" / "pkg" / "netsuke.pkg", b"pkg-windows")

    assets = module.discover_assets(dist, bin_name="netsuke")

    assert [
        asset.asset_name for asset in assets if asset.asset_name.endswith(".pkg")
    ] == [
        "linux__pkg-netsuke.pkg",
        "windows__pkg-netsuke.pkg",
    ]


def test_discover_assets_rejects_empty_files(
    module: ModuleType, tmp_path: Path
) -> None:
    """It refuses to publish zero-byte artefacts."""
    dist = tmp_path / "dist"
    create_file(dist / "linux" / "netsuke", b"")

    with pytest.raises(module.AssetError) as exc:
        module.discover_assets(dist, bin_name="netsuke")

    assert "is empty" in str(exc.value)


@pytest.mark.parametrize(
    ("value", "expected"),
    [
        (True, True),
        (False, False),
        ("true", True),
        ("false", False),
        ("YES", True),
        ("no", False),
        ("1", True),
        ("0", False),
        (" on ", True),
        (" off ", False),
        ("", False),
    ],
)
def test_coerce_bool_handles_common_inputs(
    module: ModuleType, value: object, expected: object
) -> None:
    """The coercion helper accepts boolean strings in various casings."""
    assert module._coerce_bool(value) is expected


def test_coerce_bool_rejects_unknown_input(module: ModuleType) -> None:
    """Unexpected values surface a descriptive error."""
    with pytest.raises(ValueError, match="Cannot interpret 'maybe'"):
        module._coerce_bool("maybe")


def test_cli_dry_run_outputs_summary(module: ModuleType, tmp_path: Path) -> None:
    """It prints the planned gh commands instead of uploading during dry runs."""
    dist = tmp_path / "dist"
    create_file(dist / "linux" / "netsuke", b"binary")
    create_file(dist / "linux" / "netsuke.sha256", b"checksum")

    result = subprocess.run(  # noqa: S603  # Security: CLI invocation uses trusted arguments in tests.
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

    assert result.returncode == 0, "CLI dry-run should exit successfully"
    assert "Planned uploads:" in result.stdout, (
        "Dry-run output should include upload summary"
    )
    assert "linux-netsuke" in result.stdout, "Dry-run output should list linux binary"
    assert "linux-netsuke.sha256" in result.stdout, (
        "Dry-run output should list checksum file"
    )
    assert "[dry-run] gh release upload v1.2.3" in result.stdout, (
        "Dry-run output should show gh command"
    )
