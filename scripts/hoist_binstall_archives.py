#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# dependencies = ["cyclopts>=4.25.3,<5"]
# ///
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
validated — that it is a regular file rather than a symlink or directory,
and that its destination at the ``dist/`` root is free — before anything
moves: a duplicate target triple, missing target, missing checksum,
non-regular asset, duplicate candidate, or destination collision aborts the
release with the offending names listed, so a partial or ambiguous asset set
can never be uploaded silently. Filesystem errors during discovery are
propagated rather than misreported as missing assets. That read-only
discovery and validation half lives in ``scripts/hoist_binstall_discovery.py``;
this module owns the move transaction and the command-line entry point.

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

import shutil
import sys
import typing as typ
from pathlib import Path

# Discovery and validation live in a sibling module so each file stays within
# the repository's 400-line cap; the names are re-exported here because this
# module is the hoist's entry point.
from hoist_binstall_discovery import (
    StagedArchive,
    expected_archive_names,
    locate_archives,
)

import cyclopts
from cyclopts import App, Parameter

# This script's public ``--version`` parameter is the release version used in
# archive names, so it takes precedence over Cyclopts' built-in version flag.
app = App(config=cyclopts.config.Env("INPUT_", command=False), version_flags=[])


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
        any expected asset was missing, non-regular, ambiguous, or colliding
        (in which case nothing is moved).

    Notes
    -----
    A staging configuration defining no targets, or repeating one, propagates
    ``ValueError``. An untraversable dist tree, an unprobeable asset, or a
    validated move that fails propagates ``OSError`` (after completed moves
    are rolled back), and a move failure whose rollback cannot restore every
    file propagates ``BaseExceptionGroup``.
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


def _rollback_completed_moves(completed: list[tuple[Path, Path]]) -> None:
    """Restore completed moves to their original paths, in reverse order.

    Parameters
    ----------
    completed
        The ``(source, destination)`` pairs of every finished move.

    Notes
    -----
    A restoration that fails propagates ``OSError``; the caller decides how
    to combine it with the failure that triggered the rollback.
    """
    for source, destination in reversed(completed):
        shutil.move(destination, source)


def _move_all(dist_dir: Path, located: list[StagedArchive]) -> None:
    """Move every validated pair to the dist root, all-or-nothing.

    This function orchestrates the transaction: it performs the forward
    moves, recording each, and on any failure delegates restoration to
    :func:`_rollback_completed_moves` before re-raising, so the release root
    never retains a partial asset set.

    Parameters
    ----------
    dist_dir
        Release staging root receiving the archives.
    located
        Validated archive/sidecar pairs from :func:`locate_archives`.

    Raises
    ------
    BaseExceptionGroup
        The original failure together with the rollback failure, when the
        rollback itself could not restore every file.

    Notes
    -----
    After a successful rollback the original move failure — typically an
    ``OSError`` — is re-raised unchanged.
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
            _rollback_completed_moves(completed)
        except OSError as rollback_failure:
            # `shutil.move` reports every restoration fault as an `OSError`
            # (`shutil.Error` included), so a wider handler here would only
            # swallow control-flow exceptions that must keep propagating.
            msg = "hoist move failed and rollback could not restore every file"
            raise BaseExceptionGroup(msg, [failure, rollback_failure]) from None
        raise


@app.default
def run_hoist(
    *,
    version: typ.Annotated[str, Parameter(required=True)],
    dist_dir: Path = Path("dist"),
    staging_config: Path = Path(".github/release-staging.toml"),
    manifest: Path = Path("Cargo.toml"),
) -> int:
    """Run the archive hoist from Cyclopts arguments or ``INPUT_*`` values.

    Returns
    -------
    int
        The hoist status from :func:`hoist`.

    Examples
    --------
    ``INPUT_VERSION=1.2.3`` supplies the version, and ``--version 1.2.3``
    remains available to callers that use the command-line interface.
    """
    return hoist(dist_dir, staging_config, manifest, version)


def main(argv: list[str] | None = None) -> int:
    """Parse arguments or ``INPUT_*`` variables and return the hoist status.

    Returns
    -------
    int
        The hoist command's status.

    Raises
    ------
    TypeError
        If Cyclopts returns something other than the hoist status.

    Examples
    --------
    ``main(["--version", "1.2.3"])`` runs the release hoist for version 1.2.3.
    """
    arguments = sys.argv[1:] if argv is None else argv
    result = app(arguments, result_action="return_value")
    if not isinstance(result, int):
        message = f"Cyclopts returned {type(result).__name__}; expected int"
        raise TypeError(message)
    return result


if __name__ == "__main__":
    sys.exit(main())
