"""Hash the staged candidate so the release can vouch for it before upload.

The release needs a digest for every asset it declares, taken from the bytes
the build produced. This module computes that mapping from the staged tree: for
each declared name it finds the staged file publishing under that name and
hashes it. :mod:`scripts.release_digest_record` owns the record's shape and the
comparisons made against it; this module owns only the reading of the tree.

Splitting the two keeps each file under the repository's 400-line cap and
separates two different reasons to change: how a staged tree is read, and what
a record is allowed to mean.

Names here are the names GitHub publishes, not the spelling the tree holds.
The mapping is :func:`release_github_assets_adapter.published_asset_name`, the
same authority the declaration CLI uses, so a prerelease package is recorded
under the name a consumer downloads and not the ``~`` spelling it is staged
under.

Nothing here reaches the network.

Run via ``scripts/release_declaration_cli.py`` in the release workflow;
behavioural coverage lives in ``scripts/tests/test_release_staged_digests.py``.
"""

import typing as typ

import hoist_binstall_discovery
import release_digest_files
import release_github_assets_adapter
from release_package_formats import StagedTreeError

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import pathlib


class StagedAssetError(StagedTreeError):
    """Report a staged tree that cannot yield the record the release needs.

    A :class:`release_package_formats.StagedTreeError` because the fault is in
    the tree, not in a file the operator typed: the build promised an asset and
    the tree disagrees. The declaration CLI already maps that family to its
    missing-assets exit code, so these refusals need no new branch there.
    """


class UnstagedAssetError(StagedAssetError):
    """Report a declared asset that no staged file publishes.

    The archive half of a declaration comes from configuration, so this is the
    check that catches a configured target whose archive was never built — a
    gap the operating-system packages cannot have, because their names are read
    from files that already exist.
    """


class AmbiguousStagedAssetError(StagedAssetError):
    """Report one published name claimed by two staged files.

    The record would hold one digest for two files and one of them would never
    be compared against anything, so which of them the release publishes has to
    be decided before the record is written rather than after an upload.
    """


def staged_digests(
    dist_dir: pathlib.Path, *, expected: cabc.Iterable[str]
) -> dict[str, str]:
    """Return each declared asset's digest, taken from the staged tree.

    Parameters
    ----------
    dist_dir
        Release staging root holding the artefacts the build produced.
    expected
        Asset names the release declares.

    Returns
    -------
    dict[str, str]
        Published name to the lowercase hexadecimal SHA-256 of the staged file
        that publishes under it, sorted by name.

    Notes
    -----
    Two refusals reach the caller from the helpers below, both
    :class:`StagedAssetError`: :class:`UnstagedAssetError` when a declared name
    matches no staged file, and :class:`AmbiguousStagedAssetError` when two
    files publish under one declared name. Neither is raised here, so the
    hierarchy is named in prose rather than in a ``Raises`` section, which
    Ruff's DOC501 reserves for exceptions this function raises itself.

    Only the declared names are recorded. The staged tree also holds the
    binaries, completions, manual pages and licence that ship *inside* the
    packages, and those are not release assets; a rule broad enough to admit
    them admits files under names another asset already claims.

    The tree is walked rather than listed, because the packages sit below the
    root until the hoist relocates the archives. Two files at different depths
    are still ambiguous when they publish under one name, and refusing that is
    what lets the record hold one digest per name.

    Examples
    --------
    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     root = Path(tmp)
    ...     staged = root / "netsuke.tar.gz"
    ...     _ = staged.write_bytes(b"netsuke")
    ...     recorded = staged_digests(root, expected=["netsuke.tar.gz"])
    ...     recorded["netsuke.tar.gz"] == release_digest_files.digest_of_file(staged)
    True
    """
    names = sorted(set(expected))
    indexed = _staged_index(dist_dir)
    _require_every_name_staged(indexed, names)
    _require_one_file_per_name(indexed, names)
    return {
        name: release_digest_files.digest_of_file(indexed[name][0]) for name in names
    }


def _staged_index(dist_dir: pathlib.Path) -> dict[str, list[pathlib.Path]]:
    """Index the staged tree by the name each file publishes under.

    Returns
    -------
    dict[str, list[pathlib.Path]]
        Published name to every staged file claiming it, in walk order.

    Notes
    -----
    A list rather than a single path, so two files publishing under one name
    can be reported as ambiguous instead of one quietly winning.
    """
    indexed: dict[str, list[pathlib.Path]] = {}
    for path in hoist_binstall_discovery.walk_files(dist_dir):
        published = release_github_assets_adapter.published_asset_name(path.name)
        indexed.setdefault(published, []).append(path)
    return indexed


def _require_every_name_staged(
    indexed: cabc.Mapping[str, list[pathlib.Path]], names: cabc.Sequence[str]
) -> None:
    """Refuse a declared name that the staged tree does not produce.

    Raises
    ------
    UnstagedAssetError
        If any declared name is absent from ``indexed``.
    """
    unstaged = [name for name in names if name not in indexed]
    if not unstaged:
        return
    msg = "declared assets were not staged: " + ", ".join(
        repr(name) for name in unstaged
    )
    raise UnstagedAssetError(msg)


def _require_one_file_per_name(
    indexed: cabc.Mapping[str, list[pathlib.Path]], names: cabc.Sequence[str]
) -> None:
    """Refuse a declared name that more than one staged file claims.

    Raises
    ------
    AmbiguousStagedAssetError
        If any declared name is claimed by more than one file.
    """
    claims = {
        name: [path.as_posix() for path in indexed[name]]
        for name in names
        if len(indexed[name]) > 1
    }
    if not claims:
        return
    msg = "several staged files publish under one declared name: " + ", ".join(
        f"{name!r} <- {paths}" for name, paths in sorted(claims.items())
    )
    raise AmbiguousStagedAssetError(msg)
