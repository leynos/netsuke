"""Validate a release's staged files against the set it declared.

A release is only as trustworthy as the agreement between what it said it
would publish and what it actually presents. This module states that agreement
as exact set equality: the staged names are precisely the declared names, each
appearing once, each a non-empty regular file, and each a bare file name that
cannot resolve outside the directory holding it.

Exact equality is the point. Checking only that every present file looks
acceptable accepts a release that is *missing* an asset, as long as what did
arrive is well formed. On 2026-10-12 every published netsuke release carried
its six archives' ``.sha256`` sidecars and none of the archives they describe:
every file present passed inspection, and the release was unusable through
``cargo binstall``. Comparing against the declared set catches it.

Nothing here performs I/O beyond reading a directory listing, and every
rejection carries a diagnosis rather than an exit code, because
:func:`validate_inventory` raises one error per class.

Run via the release workflow's inventory verification; behavioural coverage
lives in ``scripts/tests/test_release_inventory.py``.
"""

import dataclasses
import os
import typing as typ
from pathlib import PurePosixPath, PureWindowsPath

import hoist_binstall_discovery

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    from pathlib import Path

#: Appended to an archive's name to describe its checksum sidecar.
SIDECAR_SUFFIX = ".sha256"


class InventoryError(ValueError):
    """Report that a staged inventory disagrees with the declared one."""


class MissingAssetError(InventoryError):
    """Report a declared asset the release did not stage."""


class UnexpectedAssetError(InventoryError):
    """Report a staged asset the release never declared."""


class EmptyAssetError(InventoryError):
    """Report a staged asset carrying no bytes, which uploads uselessly."""


class DuplicateAssetError(InventoryError):
    """Report two staged entries claiming one asset name."""


class EscapingAssetNameError(InventoryError):
    """Report a name that could resolve outside the directory holding it."""


class MalformedAssetNameError(InventoryError):
    """Report a name that is not a bare file name."""


@dataclasses.dataclass(frozen=True, slots=True)
class PublishedAsset:
    """One file a release presents for publication.

    Parameters
    ----------
    name
        Bare file name as the uploader would publish it.
    size
        Size in bytes, as reported by the directory listing.

    Notes
    -----
    The dataclass validates nothing. A name is judged by
    :func:`validate_inventory` against the declared set, so every rejection is
    reported with its context rather than at construction.
    """

    name: str
    size: int


def _walks_out_of_its_directory(parts: tuple[str, ...]) -> bool:
    """Report whether walking ``parts`` in order ever goes above the start.

    Parameters
    ----------
    parts
        Path components, already split by whichever flavour produced them.

    Returns
    -------
    bool
        ``True`` when a ``..`` component is reached with no preceding
        component left to cancel it, so the path resolves above its origin.

    Notes
    -----
    Counting depth is what catches ``a/../../b``: the leading ``a`` looks
    harmless and the traversal happens two components later.
    """
    depth = 0
    for part in parts:
        if part == "..":
            depth -= 1
            if depth < 0:
                return True
        elif part not in {"", "."}:
            depth += 1
    return False


def _escapes_its_directory(name: str) -> bool:
    r"""Report whether ``name`` could resolve outside the directory holding it.

    Parameters
    ----------
    name
        Candidate asset name to judge.

    Returns
    -------
    bool
        ``True`` for an absolute path, a Windows drive or UNC path, or any
        component sequence that walks above its origin.

    Notes
    -----
    Both path flavours are consulted because the name is data, not a path on
    this host. ``C:\Windows\netsuke.exe`` is one ordinary component to a POSIX
    parser and something else to an uploader on Windows; a name absolute under
    *either* reading is refused.
    """
    for flavour in (PurePosixPath, PureWindowsPath):
        parsed = flavour(name)
        if parsed.is_absolute() or _walks_out_of_its_directory(parsed.parts):
            return True
    return False


def _is_bare_file_name(name: str) -> bool:
    """Report whether ``name`` is a single, non-empty, flat component.

    Parameters
    ----------
    name
        Candidate asset name to judge.

    Returns
    -------
    bool
        ``True`` when the name is exactly one component under both path
        flavours, with no empty component and no reserved spelling such as
        ``.`` or ``..``.

    Notes
    -----
    Both flavours must agree, for the reason given in
    :func:`_escapes_its_directory`: a name nested under Windows separators
    alone is still not the flat name the uploader publishes, even though a
    POSIX parser treats the backslashes as ordinary characters.
    """
    return all(
        len(flavour(name).parts) == 1 and flavour(name).parts[0] not in {"", ".", ".."}
        for flavour in (PurePosixPath, PureWindowsPath)
    )


def _rejected_names(names: cabc.Iterable[str]) -> tuple[list[str], list[str]]:
    """Partition ``names`` into those escaping and those merely malformed.

    Parameters
    ----------
    names
        Candidate asset names, in the order to report them.

    Returns
    -------
    tuple[list[str], list[str]]
        The escaping names and the malformed names, each in input order.

    Notes
    -----
    The two are different diagnoses: ``../netsuke`` means the release tried to
    publish outside its staging directory, ``sub/netsuke`` that a nested path
    reached the listing. Reporting both as "bad name" would hide the first,
    which is the one with security consequences.
    """
    escaping: list[str] = []
    malformed: list[str] = []
    for name in names:
        if _escapes_its_directory(name):
            escaping.append(name)
        elif not _is_bare_file_name(name):
            malformed.append(name)
    return escaping, malformed


def _require_usable_entries(assets: list[PublishedAsset]) -> dict[str, int]:
    """Reject staged assets that could not be published, returning name counts.

    Returns
    -------
    dict[str, int]
        How many entries claim each staged name, which is what the caller
        compares against the declaration.

    Raises
    ------
    EmptyAssetError
        If a staged asset carries no bytes.
    DuplicateAssetError
        If two staged entries claim one name.

    Notes
    -----
    Emptiness is judged before duplication so a duplicated empty file is not
    reported as merely duplicated, which would hide that it could never be
    used.
    """
    empty = sorted({asset.name for asset in assets if asset.size == 0})
    if empty:
        msg = "staged assets carry no bytes: " + ", ".join(repr(name) for name in empty)
        raise EmptyAssetError(msg)

    counts: dict[str, int] = {}
    for asset in assets:
        counts[asset.name] = counts.get(asset.name, 0) + 1
    duplicated = sorted(name for name, count in counts.items() if count > 1)
    if duplicated:
        msg = "staged asset names are duplicated: " + ", ".join(
            repr(name) for name in duplicated
        )
        raise DuplicateAssetError(msg)
    return counts


def _require_declared_agreement(
    counts: dict[str, int], *, expected: cabc.Iterable[str]
) -> None:
    """Require the staged names and the declared names to be the same set.

    ``counts`` comes from :func:`_require_usable_entries`; ``expected`` holds
    the asset names the release declared.

    Raises
    ------
    MissingAssetError
        If a declared asset was not staged.
    UnexpectedAssetError
        If an asset was staged but never declared.

    Notes
    -----
    Absence is reported before surpluses: a release missing an asset is
    unusable while one carrying an extra file is untidy, so the diagnosis a
    maintainer needs first is the one that breaks the download.
    """
    declared = set(expected)
    staged = set(counts)
    absent = sorted(declared - staged)
    if absent:
        msg = "declared assets were not staged: " + ", ".join(
            repr(name) for name in absent
        )
        raise MissingAssetError(msg)
    undeclared = sorted(staged - declared)
    if undeclared:
        msg = "assets were staged but never declared: " + ", ".join(
            repr(name) for name in undeclared
        )
        raise UnexpectedAssetError(msg)


def validate_inventory(
    present: cabc.Iterable[PublishedAsset], *, expected: cabc.Iterable[str]
) -> None:
    """Require the staged assets to be exactly the declared ones.

    Parameters
    ----------
    present
        Assets the release actually staged, in any order.
    expected
        Asset names the release declared.

    Raises
    ------
    EscapingAssetNameError
        If a staged name could resolve outside the staging directory.
    MalformedAssetNameError
        If a staged name is not a bare file name.

    Notes
    -----
    Checks run from the most local defect outward: name shape, then emptiness,
    then duplication, then agreement with the declaration. A name is judged
    before set membership because ``..`` is both undeclared *and* escaping,
    and escaping is the diagnosis worth surfacing. Each check reports every
    offending name rather than the first, so one run describes the whole
    shortfall.

    The remaining four refusing classes are raised by the stage helpers this
    delegates to, each of which documents them: `_require_usable_entries`
    raises `EmptyAssetError` and `DuplicateAssetError`,
    `_require_declared_agreement` raises `MissingAssetError` and
    `UnexpectedAssetError`. They remain the caller's to handle, since nothing
    here catches them.
    """
    assets = list(present)
    escaping, malformed = _rejected_names(asset.name for asset in assets)
    if escaping:
        msg = (
            "staged asset names could resolve outside the staging directory: "
            + ", ".join(repr(name) for name in escaping)
        )
        raise EscapingAssetNameError(msg)
    if malformed:
        msg = "staged asset names are not bare file names: " + ", ".join(
            repr(name) for name in malformed
        )
        raise MalformedAssetNameError(msg)

    counts = _require_usable_entries(assets)
    _require_declared_agreement(counts, expected=expected)


def binstall_asset_names(
    staging_config: Path, manifest: Path, version: str
) -> tuple[str, ...]:
    """Declare the archive and sidecar each staged target contributes.

    Parameters
    ----------
    staging_config
        Path to ``.github/release-staging.toml``.
    manifest
        Path to the Cargo manifest naming the package.
    version
        Release version interpolated into each archive name.

    Returns
    -------
    tuple[str, ...]
        Each target's archive immediately followed by its
        ``.sha256`` sidecar.

    Notes
    -----
    The archive names are not derived here. They come from
    :func:`hoist_binstall_discovery.expected_archive_names`, so the inventory
    and the hoist cannot drift apart about what an archive is called; this
    function adds only the sidecar suffix that the hoist's own staging step
    emits beside each archive.
    """
    archives = hoist_binstall_discovery.expected_archive_names(
        staging_config, manifest, version
    )
    return tuple(
        name for archive in archives for name in (archive, f"{archive}{SIDECAR_SUFFIX}")
    )


def read_inventory(directory: Path) -> list[PublishedAsset]:
    """Read the bare name and byte size of every regular file in ``directory``.

    Parameters
    ----------
    directory
        Staging directory to list.

    Returns
    -------
    list[PublishedAsset]
        One entry per regular file, sorted by name.

    Notes
    -----
    Only regular files are reported, and symlinks are not followed, so a link
    is never mistaken for the asset it points at: the release must publish the
    archive itself, not a reference to a file left elsewhere. Sorted output
    keeps the result stable, so a caller comparing two listings compares
    content and not directory order.
    """
    with os.scandir(directory) as entries:
        assets = [
            PublishedAsset(name=entry.name, size=entry.stat().st_size)
            for entry in entries
            if entry.is_file(follow_symlinks=False)
        ]
    return sorted(assets, key=lambda asset: asset.name)
