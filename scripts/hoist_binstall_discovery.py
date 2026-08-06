"""Discovery and validation for the cargo-binstall hoist.

This module owns the read-only half of the hoist: deriving the expected
archive set from the repository's configuration files, walking the downloaded
artefact tree, and deciding whether every expected archive and sidecar is
present, regular, unambiguous, and free to move. Nothing here mutates the
filesystem — the move transaction lives in
``scripts/hoist_binstall_archives.py``, which imports this module.

Splitting the two halves keeps each file within the repository's 400-line
cap and puts a hard seam between validation and mutation: validation is
complete before the first move begins.
"""

from __future__ import annotations

import os
import stat
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


def _validate_targets(staging_config: Path, targets: list[str]) -> None:
    """Reject a target list that cannot yield a unique archive per target.

    Parameters
    ----------
    staging_config
        Path the targets were read from, named in any error message.
    targets
        Target triples read from the staging configuration.

    Raises
    ------
    ValueError
        If the list is empty, or repeats a triple. Two entries sharing a
        triple would derive the same archive name twice, so the same staged
        file would be claimed and moved twice: the second move would fail on
        the already-relocated source and roll back an otherwise valid
        release. Reject the ambiguity at its source.
    """
    if not targets:
        msg = f"{staging_config} defines no release targets"
        raise ValueError(msg)
    duplicates = sorted({target for target in targets if targets.count(target) > 1})
    if duplicates:
        msg = f"{staging_config} defines duplicate release targets: " + ", ".join(
            duplicates
        )
        raise ValueError(msg)


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
        If the staging configuration defines no release targets, or defines
        the same target triple more than once.
    OSError
        If either configuration file cannot be read.
    tomllib.TOMLDecodeError
        If either configuration file is not valid TOML.
    """
    config = tomllib.loads(staging_config.read_text(encoding="utf-8"))
    package = tomllib.loads(manifest.read_text(encoding="utf-8"))["package"]["name"]
    targets = [entry["target"] for entry in config["targets"].values()]
    _validate_targets(staging_config, targets)
    return [f"{package}-{version}-{target}.tar.gz" for target in sorted(targets)]


def _is_file(path: Path) -> bool:
    """Report whether ``path`` is a regular file, surfacing unexpected errors.

    The probe uses ``lstat`` so a symlink is never mistaken for the regular
    file it points at: a release asset must be the archive itself, not a link
    that ``shutil.move`` would relocate while leaving its target behind.

    Parameters
    ----------
    path
        Path to probe.

    Returns
    -------
    bool
        ``True`` for an existing regular file; ``False`` when the path is
        absent, a symlink, or any other entry type.

    Raises
    ------
    OSError
        For any probe failure other than the path being absent, so an
        unreadable asset is never misreported as a missing one.
    """
    try:
        return stat.S_ISREG(path.lstat().st_mode)
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


def _destination_collisions(dist_dir: Path, names: tuple[str, ...]) -> list[str]:
    """Report which of ``names`` is already occupied at the dist root.

    Parameters
    ----------
    dist_dir
        Release staging root the pair would be moved into.
    names
        Bare file names whose destinations must be free.

    Returns
    -------
    list[str]
        The occupied names, in the order given; empty when both are free.

    Raises
    ------
    OSError
        If a destination cannot be probed.
    """
    return [name for name in names if _exists_any(dist_dir / name)]


def _resolve_archive(
    dist_dir: Path, staged: list[Path], name: str
) -> StagedArchive | str:
    """Resolve one expected archive, or describe why it cannot be staged.

    Every rejection reason is returned rather than raised so the caller can
    report the whole shortfall at once instead of aborting at the first
    problem.

    Parameters
    ----------
    dist_dir
        Release staging root; destinations are probed directly below it.
    staged
        Every file found below the dist root, excluding the root itself.
    name
        Expected archive file name to resolve.

    Returns
    -------
    StagedArchive | str
        The resolved pair, or a human-readable description of the failure.

    Raises
    ------
    OSError
        If an asset or destination cannot be probed.
    """
    matches = [path for path in staged if path.name == name]
    if len(matches) != 1:
        return f"{name} (found {len(matches)} candidates)"
    archive = matches[0]
    if not _is_file(archive):
        return f"{name} (not a regular file)"
    sidecar = archive.with_name(f"{name}.sha256")
    if not _is_file(sidecar):
        return f"{name}.sha256 (checksum sidecar absent)"
    collisions = _destination_collisions(dist_dir, (name, sidecar.name))
    if collisions:
        return f"{name} (destination already occupied: {collisions})"
    return StagedArchive(name=name, archive=archive, sidecar=sidecar)


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
        missing, non-regular, ambiguous, or destination-colliding asset
        descriptions.

    Raises
    ------
    OSError
        If the dist tree cannot be traversed or an asset cannot be probed.
    """
    staged = [path for path in _walk_files(dist_dir) if path.parent != dist_dir]
    located: list[StagedArchive] = []
    missing: list[str] = []
    for name in names:
        resolved = _resolve_archive(dist_dir, staged, name)
        if isinstance(resolved, StagedArchive):
            located.append(resolved)
        else:
            missing.append(resolved)
    return located, missing
