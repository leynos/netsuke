"""Validate in-memory release inventory observations against declared names.

A release is only as trustworthy as the agreement between what it said it
would publish and what it actually presents. This module states that agreement
as exact set equality: the staged names are precisely the declared names, each
appearing once, each observation non-empty, and each name a bare file name
that cannot resolve outside the directory holding it. The CLI constructs
observations only for regular files and does not follow symbolic links.

Exact equality is the point. Checking only that every present file looks
acceptable accepts a release that is *missing* an asset, as long as what did
arrive is well formed. On 2026-10-12 every published netsuke release carried
its six archives' ``.sha256`` sidecars and none of the archives they describe:
every file present passed inspection, and the release was unusable through
``cargo binstall``. Comparing against the declared set catches it.

Directory scanning and archive-name discovery belong to the release adapters.
This module accepts observed names and sizes, and every rejection carries a
diagnosis rather than an exit code.

Run via the release workflow's inventory verification; behavioural coverage
lives in ``scripts/tests/test_release_inventory.py``.
"""

import dataclasses
import typing as typ

import release_asset_names

if typ.TYPE_CHECKING:
    import collections.abc as cabc

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

    Construction validates nothing: a name is judged by
    :func:`validate_inventory` against the declared set, so every rejection
    carries its context rather than arriving here.

    Parameters
    ----------
    name
        Bare file name as the uploader would publish it.
    size
        Size in bytes, as reported by the directory listing.
    """

    name: str
    size: int


def _require_safe_names(assets: list[PublishedAsset]) -> None:
    """Refuse staged names that are not bare file names inside the directory.

    Raises
    ------
    EscapingAssetNameError
        If a staged name could resolve outside the staging directory.
    MalformedAssetNameError
        If a staged name is not a bare file name.

    Notes
    -----
    Escaping is reported before malformed, because ``..`` is both undeclared
    and escaping, and the security diagnosis is the one worth surfacing. Each
    refusal names every offender rather than the first, so one run describes
    the whole shortfall.

    Examples
    --------
    >>> _require_safe_names([PublishedAsset(name="netsuke.tar.gz", size=1)])
    >>> try:
    ...     _require_safe_names([PublishedAsset(name="../x", size=1)])
    ... except EscapingAssetNameError:
    ...     print("refused")
    refused
    """
    escaping, malformed = release_asset_names.rejected_names(
        asset.name for asset in assets
    )
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


def _require_usable_entries(assets: list[PublishedAsset]) -> dict[str, int]:
    """Reject staged assets that could not be published, returning name counts.

    Returns
    -------
    dict[str, int]
        How many entries claim each staged name.

    Notes
    -----
    Emptiness is judged before duplication so a duplicated empty file is not
    reported as merely duplicated, which would hide that it could never be
    used. :class:`EmptyAssetError` is raised by :func:`_require_non_empty` and
    :class:`DuplicateAssetError` by :func:`_require_unique`.
    """
    _require_non_empty(assets)
    counts = _name_counts(assets)
    _require_unique(counts)
    return counts


def _require_non_empty(assets: list[PublishedAsset]) -> None:
    """Refuse assets that carry no bytes, naming every one of them.

    Raises
    ------
    EmptyAssetError
        If any staged asset has size zero.
    """
    empty = sorted({asset.name for asset in assets if asset.size == 0})
    if not empty:
        return
    msg = "staged assets carry no bytes: " + ", ".join(repr(name) for name in empty)
    raise EmptyAssetError(msg)


def _name_counts(assets: cabc.Iterable[PublishedAsset]) -> dict[str, int]:
    """Return how many entries claim each staged name.

    Returns
    -------
    dict[str, int]
        Each staged name mapped to the number of entries claiming it.

    Examples
    --------
    >>> assets = [PublishedAsset("a", 1), PublishedAsset("a", 2)]
    >>> _name_counts(assets)
    {'a': 2}
    """
    counts: dict[str, int] = {}
    for asset in assets:
        counts[asset.name] = counts.get(asset.name, 0) + 1
    return counts


def _require_unique(counts: cabc.Mapping[str, int]) -> None:
    """Refuse two staged entries claiming one asset name.

    Raises
    ------
    DuplicateAssetError
        If any name is claimed more than once.
    """
    duplicated = sorted(name for name, count in counts.items() if count > 1)
    if not duplicated:
        return
    msg = "staged asset names are duplicated: " + ", ".join(
        repr(name) for name in duplicated
    )
    raise DuplicateAssetError(msg)


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
    absent = _missing_assets(declared, staged)
    if absent:
        msg = "declared assets were not staged: " + ", ".join(
            repr(name) for name in absent
        )
        raise MissingAssetError(msg)
    undeclared = _unexpected_assets(staged, declared)
    if undeclared:
        msg = "assets were staged but never declared: " + ", ".join(
            repr(name) for name in undeclared
        )
        raise UnexpectedAssetError(msg)


def _missing_assets(declared: set[str], staged: set[str]) -> list[str]:
    """Return declared names that the staged tree does not contain.

    Returns
    -------
    list[str]
        Missing asset names in sorted order.

    Examples
    --------
    >>> _missing_assets({"a", "b"}, {"a"})
    ['b']
    """
    return sorted(declared - staged)


def _unexpected_assets(staged: set[str], declared: set[str]) -> list[str]:
    """Return staged names absent from the release declaration.

    Returns
    -------
    list[str]
        Unexpected asset names in sorted order.

    Examples
    --------
    >>> _unexpected_assets({"a", "extra"}, {"a"})
    ['extra']
    """
    return sorted(staged - declared)


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

    Notes
    -----
    Checks run from the most local defect outward: name shape, then emptiness,
    then duplication, then agreement with the declaration. A name is judged
    before set membership because ``..`` is both undeclared *and* escaping,
    and escaping is the diagnosis worth surfacing. Each check reports every
    offending name rather than the first, so one run describes the whole
    shortfall. Nothing here catches the helpers' refusals: whichever subclass
    of :class:`InventoryError` the first failing check reports reaches the
    caller unchanged.

    Examples
    --------
    >>> staged = [PublishedAsset(name="netsuke.tar.gz", size=11)]
    >>> validate_inventory(staged, expected=["netsuke.tar.gz"])
    >>> validate_inventory(staged, expected=["netsuke.tar.gz", "netsuke.tar.gz.sha256"])
    Traceback (most recent call last):
        ...
    release_inventory.MissingAssetError: declared assets were not staged: \
'netsuke.tar.gz.sha256'
    """
    assets = list(present)
    _require_safe_names(assets)
    counts = _require_usable_entries(assets)
    _require_declared_agreement(counts, expected=expected)
