#!/usr/bin/env -S uv run python
# /// script
# requires-python = ">=3.13"
# dependencies = ["cyclopts>=2.9", "plumbum"]
# ///

"""Upload packaged release artefacts to a GitHub release.

The script discovers artefacts in a staging directory, validates their
filenames and sizes, and optionally uploads them using the GitHub CLI. It is
idempotent and supports a dry-run mode used by the release dry-run workflow to
assert expected asset names without mutating state.

Examples
--------
Upload artefacts to the ``v1.2.3`` release::

    upload_release_assets --release-tag v1.2.3 --bin-name netsuke

Inspect the planned uploads without publishing anything::

    upload_release_assets --release-tag v1.2.3 --bin-name netsuke --dry-run
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Annotated, Iterable
import sys

import cyclopts
from cyclopts import App, Parameter
from plumbum import local


class AssetError(RuntimeError):
    """Raised when the staged artefacts are invalid."""


@dataclass(frozen=True)
class ReleaseAsset:
    """Artefact staged for upload to a GitHub release."""

    path: Path
    asset_name: str
    size: int


app = App(config=cyclopts.config.Env("INPUT_", command=False))


def _is_candidate(path: Path, bin_name: str) -> bool:
    name = path.name
    if name in {bin_name, f"{bin_name}.exe", f"{bin_name}.1"}:
        return True
    if name.endswith(".sha256"):
        return True
    return path.suffix in {".deb", ".rpm", ".pkg", ".msi"}


def _resolve_asset_name(path: Path) -> str:
    suffix = path.suffix.lower()
    if suffix in {".deb", ".rpm", ".pkg"}:
        return path.name
    return f"{path.parent.name}-{path.name}"


def discover_assets(dist_dir: Path, *, bin_name: str) -> list[ReleaseAsset]:
    """Return the artefacts that should be published.

    Parameters
    ----------
    dist_dir:
        Root directory that contains the staged artefacts.
    bin_name:
        Binary name used to match platform-specific artefacts.

    Returns
    -------
    list[ReleaseAsset]
        Ordered collection of artefacts ready to upload.

    Raises
    ------
    AssetError
        If no artefacts are found, an artefact is empty, or multiple files would
        upload with the same asset name.
    """

    if not dist_dir.exists():
        raise AssetError(f"Artefact directory {dist_dir} does not exist")

    assets: list[ReleaseAsset] = []
    seen: dict[str, Path] = {}

    for path in sorted(p for p in dist_dir.rglob("*") if p.is_file()):
        if not _is_candidate(path, bin_name):
            continue
        size = path.stat().st_size
        if size <= 0:
            raise AssetError(f"Artefact {path} is empty")
        asset_name = _resolve_asset_name(path)
        previous = seen.get(asset_name)
        if previous:
            raise AssetError(
                "Asset name collision: "
                f"{asset_name} would upload both {previous} and {path}"
            )
        seen[asset_name] = path
        assets.append(ReleaseAsset(path=path, asset_name=asset_name, size=size))

    if not assets:
        raise AssetError(f"No artefacts discovered in {dist_dir}")

    return assets


def _render_summary(assets: Iterable[ReleaseAsset]) -> str:
    lines = ["Planned uploads:"]
    for asset in assets:
        lines.append(
            f"  - {asset.asset_name} ({asset.size} bytes) -> {asset.path}"
        )
    return "\n".join(lines)


def upload_assets(
    *, release_tag: str, assets: Iterable[ReleaseAsset], dry_run: bool = False
) -> None:
    """Upload artefacts to GitHub using the ``gh`` CLI."""

    gh_cmd = None
    for asset in assets:
        descriptor = f"{asset.path}#{asset.asset_name}"
        if dry_run:
            print(f"[dry-run] gh release upload {release_tag} {descriptor} --clobber")
            continue
        if gh_cmd is None:
            gh_cmd = local["gh"]
        gh_cmd[
            "release",
            "upload",
            release_tag,
            descriptor,
            "--clobber",
        ]()


def main(
    *,
    release_tag: str,
    bin_name: str,
    dist_dir: Path = Path("dist"),
    dry_run: bool = False,
) -> int:
    """Entry point shared by the CLI and tests."""

    try:
        assets = discover_assets(dist_dir, bin_name=bin_name)
    except AssetError as exc:
        print(exc, file=sys.stderr)
        return 1

    if dry_run:
        print(_render_summary(assets))

    try:
        upload_assets(release_tag=release_tag, assets=assets, dry_run=dry_run)
    except Exception as exc:  # pragma: no cover - surfaced by plumbum
        print(exc, file=sys.stderr)
        return 1

    return 0


@app.default
def cli(
    *,
    release_tag: Annotated[str, Parameter(required=True)],
    bin_name: Annotated[str, Parameter(required=True)],
    dist_dir: Path = Path("dist"),
    dry_run: bool = False,
) -> int:
    """Cyclopts-bound CLI entry point."""

    return main(
        release_tag=release_tag,
        bin_name=bin_name,
        dist_dir=dist_dir,
        dry_run=dry_run,
    )


if __name__ == "__main__":  # pragma: no cover - exercised via CLI
    raise SystemExit(app())
