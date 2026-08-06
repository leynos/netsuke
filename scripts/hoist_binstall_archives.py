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
validated — including that its destination at the ``dist/`` root is free —
before anything moves: a missing target, missing checksum, duplicate
candidate, or destination collision aborts the release with the offending
names listed, so a partial or ambiguous asset set can never be uploaded
silently. Filesystem errors during discovery are propagated rather than
misreported as missing assets.

The move phase is all-or-nothing: validation completes before anything
moves, and if any move then fails, every completed move is rolled back to
its original nested path before the failure propagates. When rollback
itself also fails, both failures are surfaced together rather than the
original cause being lost. Destinations are never overwritten: any
pre-existing entry at a destination — file, directory, or symlink —
is reported as a collision during validation.

Run via ``.github/workflows/release.yml``; behavioural coverage lives in
``tests/workflow_contracts/hoist_binstall_archives_test.py``.
"""

from __future__ import annotations

import argparse
import os
import shutil
import stat
import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class StagedArchive:
    """One expected archive with its resolved staging locations.

    Parameters
    ----------
    name
        Bare archive file name, e.g. ``pkg-1.2.3-x86_64.tar.gz``.
    archive
        Resolved path of the staged archive below the dist root.
    sidecar
        Resolved path of the archive's ``.sha256`` checksum sidecar.
    """

    name: str
    archive: Path
    sidecar: Path


def expected_archive_names(
    staging_config: Path, manifest: Path, version: str
) -> list[str]:
    """Derive the expected archive file names for every staged target.

    Parameters
    ----------
    staging_config
        Path to the staging configuration
        (``.github/release-staging.toml``); its ``[targets.*]`` tables supply
        the target triples.
    manifest
        Path to the Cargo manifest supplying the package name.
    version
        Release version interpolated into each archive name.

    Returns
    -------
    list[str]
        Sorted ``{package}-{version}-{target}.tar.gz`` names, one per target.

    Raises
    ------
    ValueError
        If the staging configuration defines no release targets.
    OSError
        If either configuration file cannot be read.
    tomllib.TOMLDecodeError
        If either configuration file is not valid TOML.
    """
    config = tomllib.loads(staging_config.read_text(encoding="utf-8"))
    package = tomllib.loads(manifest.read_text(encoding="utf-8"))["package"]["name"]
    targets = [entry["target"] for entry in config["targets"].values()]
    if not targets:
        msg = f"{staging_config} defines no release targets"
        raise ValueError(msg)
    return [f"{package}-{version}-{target}.tar.gz" for target in sorted(targets)]


def _is_file(path: Path) -> bool:
    """Report whether ``path`` is a regular file, surfacing unexpected errors.

    Parameters
    ----------
    path
        Path to probe.

    Returns
    -------
    bool
        ``True`` for an existing regular file; ``False`` when the path is
        absent.

    Raises
    ------
    OSError
        For any probe failure other than the path being absent, so an
        unreadable asset is never misreported as a missing one.
    """
    try:
        return stat.S_ISREG(path.stat().st_mode)
    except FileNotFoundError:
        return False


def _exists_any(path: Path) -> bool:
    """Report whether any filesystem entry occupies ``path``.

    The probe uses ``lstat`` so symlinks — including dangling ones — count
    as occupying the destination, and directories, sockets, FIFOs, and
    device nodes are all treated the same as regular files.

    Parameters
    ----------
    path
        Destination path to probe.

    Returns
    -------
    bool
        ``True`` when any entry exists at ``path``; ``False`` when absent.

    Raises
    ------
    OSError
        For any probe failure other than the path being absent.
    """
    try:
        path.lstat()
    except FileNotFoundError:
        return False
    return True


def _walk_files(root: Path) -> list[Path]:
    """Collect every file below ``root``, propagating traversal errors.

    ``Path.rglob`` suppresses errors such as ``PermissionError`` during
    traversal, which would let an unreadable directory masquerade as a
    missing asset; ``os.walk`` with an error callback surfaces them instead.

    Parameters
    ----------
    root
        Directory to walk recursively.

    Returns
    -------
    list[Path]
        Every directory entry found below ``root``.

    Raises
    ------
    OSError
        If any directory in the tree cannot be read.
    """

    def _raise(error: OSError) -> None:
        raise error

    files: list[Path] = []
    for directory, _subdirs, names in os.walk(root, onerror=_raise):
        files.extend(Path(directory) / name for name in names)
    return files


def locate_archives(
    dist_dir: Path, names: list[str]
) -> tuple[list[StagedArchive], list[str]]:
    """Locate each named archive (and sidecar) below the dist root.

    Validation is complete before any movement: nothing is moved here, and a
    problem with any single asset is reported alongside the others rather
    than aborting at the first.

    Parameters
    ----------
    dist_dir
        Release staging root the workflow artefacts were downloaded into.
    names
        Expected archive file names from :func:`expected_archive_names`.

    Returns
    -------
    tuple[list[StagedArchive], list[str]]
        The archives resolved to unique staged locations, and the list of
        missing, ambiguous, or destination-colliding asset descriptions.

    Raises
    ------
    OSError
        If the dist tree cannot be traversed or an asset cannot be probed.
    """
    staged = [path for path in _walk_files(dist_dir) if path.parent != dist_dir]
    located: list[StagedArchive] = []
    missing: list[str] = []
    for name in names:
        matches = [path for path in staged if path.name == name]
        if len(matches) != 1:
            missing.append(f"{name} (found {len(matches)} candidates)")
            continue
        archive = matches[0]
        sidecar = archive.with_name(f"{name}.sha256")
        if not _is_file(sidecar):
            missing.append(f"{name}.sha256 (checksum sidecar absent)")
            continue
        collisions = [
            destination.name
            for destination in (dist_dir / name, dist_dir / sidecar.name)
            if _exists_any(destination)
        ]
        if collisions:
            missing.append(f"{name} (destination already occupied: {collisions})")
            continue
        located.append(StagedArchive(name=name, archive=archive, sidecar=sidecar))
    return located, missing


def hoist(dist_dir: Path, staging_config: Path, manifest: Path, version: str) -> int:
    """Validate and move every expected archive pair to the dist root.

    Parameters
    ----------
    dist_dir
        Release staging root containing the downloaded workflow artefacts.
    staging_config
        Path to ``.github/release-staging.toml``.
    manifest
        Path to the Cargo manifest supplying the package name.
    version
        Release version interpolated into the expected archive names.

    Returns
    -------
    int
        ``0`` when every archive pair was validated and moved; ``1`` when
        any expected asset was missing, ambiguous, or colliding (in which
        case nothing is moved).

    Raises
    ------
    OSError
        If the dist tree cannot be traversed, an asset cannot be probed, or
        a validated move fails (after completed moves are rolled back).
    ExceptionGroup
        If a move fails and the rollback also cannot restore every file.
    """
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
    _move_all(dist_dir, located)
    print(f"Hoisted {len(located)} archive/sidecar pairs to {dist_dir}/")
    return 0


def _move_all(dist_dir: Path, located: list[StagedArchive]) -> None:
    """Move every validated pair to the dist root, all-or-nothing.

    Completed moves are recorded so that a failure part-way through restores
    every already-moved file to its original nested path before the failure
    propagates; the release root never retains a partial asset set.

    Parameters
    ----------
    dist_dir
        Release staging root receiving the archives.
    located
        Validated archive/sidecar pairs from :func:`locate_archives`.

    Raises
    ------
    OSError
        The original move failure, after a successful rollback.
    ExceptionGroup
        The original failure together with the rollback failure, when the
        rollback itself could not restore every file.
    """
    completed: list[tuple[Path, Path]] = []
    try:
        for entry in located:
            for source in (entry.archive, entry.sidecar):
                destination = dist_dir / source.name
                shutil.move(source, destination)
                completed.append((source, destination))
    except BaseException as failure:
        try:
            for source, destination in reversed(completed):
                shutil.move(destination, source)
        except BaseException as rollback_failure:
            raise BaseExceptionGroup(
                "hoist move failed and rollback could not restore every file",
                [failure, rollback_failure],
            ) from None
        raise


def main(argv: list[str] | None = None) -> int:
    """Parse command-line arguments and run the hoist.

    Parameters
    ----------
    argv
        Argument vector to parse; ``None`` lets ``argparse`` read
        ``sys.argv``.

    Returns
    -------
    int
        Process exit status from :func:`hoist`.

    Raises
    ------
    SystemExit
        If argument parsing fails (raised by ``argparse``).
    """
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
