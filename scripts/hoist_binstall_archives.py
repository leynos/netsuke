"""Hoist staged cargo-binstall archives to the release root.

The release workflow downloads one workflow artefact per target into
``dist/<artefact>/<staging-dir>/``, where ``stage-release-artefacts`` placed a
``{package}-{version}-{target}.tar.gz`` archive and its ``.sha256`` sidecar.
``upload-release-assets`` namespaces nested paths, so the archives must sit at
the ``dist/`` root to upload under the plain names the
``[package.metadata.binstall]`` template in ``Cargo.toml`` resolves.

The expected archive set is derived from ``.github/release-staging.toml``
(the target triples) and ``Cargo.toml`` (the package name), keeping those
files the single source of truth. Every expected archive and sidecar is
validated before anything moves: a missing target or checksum aborts the
release with the missing names listed, so a partial asset set can never be
uploaded silently.

Run via ``.github/workflows/release.yml``; behavioural coverage lives in
``tests/workflow_contracts/hoist_binstall_archives_test.py``.
"""

from __future__ import annotations

import argparse
import shutil
import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class StagedArchive:
    """One expected archive with its resolved staging locations."""

    name: str
    archive: Path
    sidecar: Path


def expected_archive_names(
    staging_config: Path, manifest: Path, version: str
) -> list[str]:
    """Derive the expected archive file names for every staged target."""
    config = tomllib.loads(staging_config.read_text(encoding="utf-8"))
    package = tomllib.loads(manifest.read_text(encoding="utf-8"))["package"]["name"]
    targets = [entry["target"] for entry in config["targets"].values()]
    if not targets:
        msg = f"{staging_config} defines no release targets"
        raise ValueError(msg)
    return [f"{package}-{version}-{target}.tar.gz" for target in sorted(targets)]


def locate_archives(dist_dir: Path, names: list[str]) -> tuple[list[StagedArchive], list[str]]:
    """Locate each named archive (and sidecar) below the dist root.

    Returns the resolved archives and the list of missing archive or sidecar
    names. Nothing is moved here; validation is complete before any movement.
    """
    located: list[StagedArchive] = []
    missing: list[str] = []
    for name in names:
        matches = [path for path in dist_dir.rglob(name) if path.parent != dist_dir]
        if len(matches) != 1:
            missing.append(f"{name} (found {len(matches)} candidates)")
            continue
        archive = matches[0]
        sidecar = archive.with_name(f"{name}.sha256")
        if not sidecar.is_file():
            missing.append(f"{name}.sha256 (checksum sidecar absent)")
            continue
        located.append(StagedArchive(name=name, archive=archive, sidecar=sidecar))
    return located, missing


def hoist(dist_dir: Path, staging_config: Path, manifest: Path, version: str) -> int:
    """Validate and move every expected archive pair to the dist root."""
    names = expected_archive_names(staging_config, manifest, version)
    print(f"Expected cargo-binstall archives: {', '.join(names)}")
    located, missing = locate_archives(dist_dir, names)
    for entry in located:
        print(f"Found {entry.name} at {entry.archive.parent}")
    if missing:
        print(
            "Missing cargo-binstall release assets: " + ", ".join(missing),
            file=sys.stderr,
        )
        return 1
    for entry in located:
        shutil.move(entry.archive, dist_dir / entry.archive.name)
        shutil.move(entry.sidecar, dist_dir / entry.sidecar.name)
    print(f"Hoisted {len(located)} archive/sidecar pairs to {dist_dir}/")
    return 0


def main(argv: list[str] | None = None) -> int:
    """Parse arguments and run the hoist."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True, help="Release version")
    parser.add_argument("--dist-dir", type=Path, default=Path("dist"))
    parser.add_argument(
        "--staging-config",
        type=Path,
        default=Path(".github/release-staging.toml"),
    )
    parser.add_argument("--manifest", type=Path, default=Path("Cargo.toml"))
    args = parser.parse_args(argv)
    return hoist(args.dist_dir, args.staging_config, args.manifest, args.version)


if __name__ == "__main__":
    sys.exit(main())
