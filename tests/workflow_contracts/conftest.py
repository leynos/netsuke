"""Shared fixtures and helpers for the cargo-binstall hoist test modules.

``scripts/hoist_binstall_archives.py`` moves the staged
``{package}-{version}-{target}.tar.gz`` archives (and their ``.sha256``
sidecars) from the downloaded workflow-artefact layout to the ``dist/`` root,
where ``upload-release-assets`` publishes them under the plain names the
binstall metadata in ``Cargo.toml`` resolves. The example-based, rollback,
and generated-layout suites all stage workspaces through the fixture and
helpers here, so the staging shape and the invocation seam live in one place.

Run via ``make test-workflow-contracts``.
"""

import sys
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "scripts"))

# The module under test lives in scripts/, outside any package, so the import
# cannot precede the sys.path insertion above.
import hoist_binstall_archives as hoist_mod  # ruff: ignore[module-import-not-at-top-of-file] - needs sys.path insertion

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
    """Provide a staging config, manifest, and empty dist directory.

    Parameters
    ----------
    tmp_path
        pytest-provided temporary directory.

    Returns
    -------
    dict[str, Path]
        Paths keyed by ``staging``, ``manifest``, and ``dist``.
    """
    staging = tmp_path / "release-staging.toml"
    staging.write_text(STAGING_CONFIG, encoding="utf-8")
    manifest = tmp_path / "Cargo.toml"
    manifest.write_text(MANIFEST, encoding="utf-8")
    dist = tmp_path / "dist"
    dist.mkdir()
    return {"staging": staging, "manifest": manifest, "dist": dist}


def stage_pair(
    dist: Path,
    nested_dir: str,
    name: str,
    *,
    with_sidecar: bool = True,
) -> None:
    """Create an archive (and optionally its sidecar) in the nested layout.

    Parameters
    ----------
    dist
        Dist root to stage below.
    nested_dir
        Relative ``<artefact>/<staging-dir>`` directory to stage into,
        mirroring the two nesting levels the release workflow downloads.
    name
        Archive file name to create.
    with_sidecar
        Whether to create the matching ``.sha256`` sidecar.
    """
    nested = dist / nested_dir
    nested.mkdir(parents=True, exist_ok=True)
    (nested / name).write_bytes(f"archive:{name}".encode())
    if with_sidecar:
        (nested / f"{name}.sha256").write_text(f"checksum:{name}", encoding="utf-8")


def run_hoist(workspace: dict[str, Path]) -> int:
    """Invoke the hoist against the fixture workspace.

    Parameters
    ----------
    workspace
        Fixture mapping from the :func:`workspace` fixture.

    Returns
    -------
    int
        The hoist's exit status.
    """
    return hoist_mod.hoist(
        workspace["dist"], workspace["staging"], workspace["manifest"], VERSION
    )


def assert_nothing_moved(dist: Path, names: list[str]) -> None:
    """Assert no expected asset reached the dist root.

    Parameters
    ----------
    dist
        Dist root to inspect.
    names
        Expected archive names that must not appear at the root.
    """
    for name in names:
        assert not (dist / name).exists(), (
            f"{name} must not reach the dist root on a validation failure"
        )
        assert not (dist / f"{name}.sha256").exists(), (
            f"{name}.sha256 must not reach the dist root on a validation failure"
        )
