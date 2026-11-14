"""Decide which draft release to verify and which of its assets to download.

The release workflow has to find the draft it just populated and fetch every
attachment back before it can compare those bytes with the trusted record. The
network calls belong to the workflow step that holds the token, and reading
GitHub's JSON belongs to :mod:`release_github_draft_listing`; this module
decides from the *translated* listing whether it can support the decision at
all. Its inputs are :class:`Release` and :class:`Asset` values, so a GitHub
field renamed upstream is an adapter change rather than a change to a rule
here.

Two refusals are deliberately stricter than GitHub's own behaviour. Selecting a
release requires exactly one *draft* with the requested tag, because a published
release carrying it means the draft this job was asked to verify is gone, and
two drafts do not identify one release. Planning downloads requires every asset
to be a stored, non-empty attachment with a bare, safe name: a name that could
escape the download directory would place attacker-chosen bytes beside the
trusted inputs.

Each rule is a query returning the messages for the conditions that do not
hold, and the command raises once with all of them. An operator then learns
every way the listing is unusable from one run instead of one per attempt.

Nothing here parses JSON, touches a file, or reaches the network:
:mod:`release_draft_assets_cli` owns the command, and
:mod:`release_asset_names` supplies the name judgement it shares with inventory
verification. The download plan's text format is likewise an adapter concern,
and lives in :mod:`release_download_plan`. Behavioural coverage lives in
``scripts/tests/test_release_draft_assets.py``.
"""

import collections
import dataclasses
import typing as typ

import release_asset_names

if typ.TYPE_CHECKING:
    import collections.abc as cabc


class DraftAssetError(ValueError):
    """Report a draft-release listing that cannot support a download plan."""


class ReleaseSelectionError(DraftAssetError):
    """Report a tag that does not identify exactly one draft release."""


class AssetListingError(DraftAssetError):
    """Report an asset listing whose entries cannot be downloaded safely."""


@dataclasses.dataclass(frozen=True, slots=True)
class Release:
    """Name one release as the selection rule needs to judge it.

    Parameters
    ----------
    identifier
        The release number ``gh api`` addresses its assets by.
    tag
        The git tag the release carries.
    is_draft
        Whether the release is still an unpublished draft.
    """

    identifier: int
    tag: str
    is_draft: bool


@dataclasses.dataclass(frozen=True, slots=True)
class Asset:
    """Name one release attachment as the plan rule needs to judge it.

    Parameters
    ----------
    identifier
        The asset number the download endpoint addresses.
    name
        The attachment name, as the download will write it.
    is_stored
        Whether the provider reports the upload complete.
    size
        The attachment's byte count as the provider reports it.
    """

    identifier: int
    name: str
    is_stored: bool
    size: int


def _partition_drafts(
    matching: cabc.Sequence[Release],
) -> tuple[list[Release], int]:
    """Split the tagged releases into the drafts and a count of the rest.

    Returns
    -------
    tuple[list[Release], int]
        The draft releases in listing order, and the count of the rest.
    """
    drafts = [release for release in matching if release.is_draft]
    return drafts, len(matching) - len(drafts)


def _selection_problems(
    drafts: cabc.Sequence[Release], published: int, tag: str
) -> list[str]:
    """Return every way the tagged releases fail to identify one draft.

    Returns
    -------
    list[str]
        One diagnostic per condition that holds, empty when the listing
        identifies exactly one draft carrying ``tag``.

    Notes
    -----
    Each pair is a *defect* condition and its diagnostic, so the rule reads as
    "report the defects that are present" rather than "report the guarantees
    that are missing". ``published`` is a count of the tagged releases that are
    not drafts.
    """
    defects = (
        (
            published,
            (
                f"the release tagged {tag!r} is published, not a draft, so "
                "there is no draft inventory to verify"
            ),
        ),
        (not drafts, f"no draft release is tagged {tag!r}"),
        (len(drafts) > 1, f"{len(drafts)} draft releases are tagged {tag!r}, not one"),
    )
    return [message for is_defect, message in defects if is_defect]


def select_draft_release(releases: cabc.Sequence[Release], *, tag: str) -> int:
    """Return the identifier of the one draft release carrying ``tag``.

    Parameters
    ----------
    releases
        Releases translated from a ``gh api --paginate --slurp`` listing. The
        list endpoint is read because the single-release endpoint refuses
        draft releases, which are the only ones this job may verify.
    tag
        Release tag the draft must carry.

    Returns
    -------
    int
        The draft release's identifier.

    Raises
    ------
    ReleaseSelectionError
        If ``tag`` does not identify exactly one draft.

    Examples
    --------
    >>> releases = [Release(7, "v1.2.3", is_draft=True)]
    >>> select_draft_release(releases, tag="v1.2.3")
    7
    >>> select_draft_release(releases, tag="v9.9.9")
    Traceback (most recent call last):
        ...
    release_draft_assets.ReleaseSelectionError: no draft release is tagged 'v9.9.9'
    """
    matching = [release for release in releases if release.tag == tag]
    drafts, published = _partition_drafts(matching)
    problems = _selection_problems(drafts, published, tag)
    if problems:
        raise ReleaseSelectionError("; ".join(problems))
    return drafts[0].identifier


def _quoted(names: cabc.Sequence[str]) -> str:
    """Render names for a diagnostic, quoting each one.

    Returns
    -------
    str
        The names separated by commas, each in ``repr`` form so a name holding
        whitespace or control characters cannot be misread as several names.
    """
    return ", ".join(repr(name) for name in names)


def _name_problems(names: cabc.Sequence[str]) -> list[str]:
    """Return every way the names cannot be written into the download directory.

    Returns
    -------
    list[str]
        One diagnostic per unusable group of names, empty when every name is a
        bare file name.

    Notes
    -----
    Escaping is reported before malformed, as in
    :func:`release_inventory.validate_inventory`: traversal is the diagnosis.
    """
    escaping, malformed = release_asset_names.rejected_names(names)
    groups = (
        ("could resolve outside the download directory", escaping),
        ("are not bare file names", malformed),
    )
    return [
        f"release asset names {problem}: {_quoted(group)}"
        for problem, group in groups
        if group
    ]


def _duplicate_problems(names: cabc.Sequence[str]) -> list[str]:
    """Return a diagnostic for every name two assets both claim.

    Returns
    -------
    list[str]
        One diagnostic naming every repeated name, empty when each name appears
        once.

    Notes
    -----
    A duplicate would write the second asset over the first, so only one would
    ever be verified.
    """
    counts = collections.Counter(names)
    repeated = sorted(name for name, count in counts.items() if count > 1)
    if not repeated:
        return []
    return [f"release asset names are duplicated: {_quoted(repeated)}"]


def _entry_problems(asset: Asset) -> list[str]:
    """Return every way one asset cannot be downloaded as claimed.

    Returns
    -------
    list[str]
        One diagnostic per defect, empty when the asset is usable.

    Notes
    -----
    Assets are addressed by number, and an attachment the provider has not
    finished storing, or one with no bytes, is not the artefact the trusted
    record describes.
    """
    defects = (
        (not asset.is_stored, f"{asset.name!r} is not an uploaded attachment"),
        (
            asset.size <= 0,
            f"{asset.name!r} does not report a usable byte size: {asset.size!r}",
        ),
    )
    return [message for is_defect, message in defects if is_defect]


def _usable_problems(assets: cabc.Sequence[Asset]) -> list[str]:
    """Return every way one or more assets cannot be downloaded as claimed.

    Returns
    -------
    list[str]
        One diagnostic per defect across every asset, empty when each can be
        downloaded as the trusted record describes it.
    """
    return [problem for asset in assets for problem in _entry_problems(asset)]


def plan_downloads(assets: cabc.Sequence[Asset]) -> list[Asset]:
    """Return the assets to download, in listing order.

    Parameters
    ----------
    assets
        Assets translated from a ``gh api --paginate --slurp`` listing.

    Returns
    -------
    list[Asset]
        Every asset, in listing order, once each.

    Raises
    ------
    AssetListingError
        If the listing is empty, or any name or asset cannot be downloaded as
        claimed.

    Examples
    --------
    >>> plan_downloads([Asset(11, "a.tar.gz", is_stored=True, size=5)])
    [Asset(identifier=11, name='a.tar.gz', is_stored=True, size=5)]
    """
    if not assets:
        msg = "the release lists no assets, so there is nothing to verify"
        raise AssetListingError(msg)
    names = [asset.name for asset in assets]
    problems = (
        _name_problems(names) + _duplicate_problems(names) + _usable_problems(assets)
    )
    if problems:
        msg = "the release asset listing is unusable: " + "; ".join(problems)
        raise AssetListingError(msg)
    return list(assets)
