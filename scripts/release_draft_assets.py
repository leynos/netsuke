"""Decide which draft release to verify and which of its assets to download.

The release workflow has to find the draft it just populated and fetch every
attachment back before it can compare those bytes with the trusted record. The
network calls belong to the workflow step that holds the token; this module
decides, from the already-parsed JSON ``gh api --slurp`` produced, whether that
listing can support the decision at all.

Two refusals are deliberately stricter than GitHub's own behaviour. Selecting a
release requires exactly one *draft* with the requested tag, because a published
release carrying it means the draft this job was asked to verify is gone, and
two drafts do not identify one release. Planning downloads requires every asset
to be an uploaded, non-empty attachment with a bare, safe name the tab-separated
plan can carry: a name that could escape the download directory would place
attacker-chosen bytes beside the trusted inputs.

Nothing here parses JSON, touches a file, or reaches the network:
:mod:`release_draft_assets_cli` owns those effects, and
:mod:`release_asset_names` supplies the name judgement it shares with inventory
verification. Run via ``scripts/release_draft_assets_cli.py``; behavioural
coverage lives in ``scripts/tests/test_release_draft_assets.py``.
"""

import collections
import typing as typ

import release_asset_names

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: The asset state GitHub reports for a fully uploaded attachment.
UPLOADED_STATE = "uploaded"

#: Characters that would split or truncate one line of the download plan, which
#: is written as ``id<TAB>name<NEWLINE>`` and read back field by field.
PLAN_DELIMITERS = ("\t", "\n", "\r")


class DraftAssetError(ValueError):
    """Report a draft-release listing that cannot support a download plan."""


class ReleaseSelectionError(DraftAssetError):
    """Report a tag that does not identify exactly one draft release."""


class AssetListingError(DraftAssetError):
    """Report an asset listing whose entries cannot be downloaded safely."""


def _is_integer(value: object) -> bool:
    """Report whether ``value`` is an integer rather than a boolean.

    ``bool`` subclasses ``int``, so an unguarded check would pass ``True`` to
    ``gh`` as an identifier.

    Returns
    -------
    bool
        ``True`` when the value is an integer and not a boolean.

    Examples
    --------
    >>> _is_integer(7), _is_integer(True), _is_integer("7")
    (True, False, False)
    """
    match value:
        case bool():
            return False
        case int():
            return True
        case _:
            return False


def _entries(pages: object, *, what: str) -> list[cabc.Mapping[str, object]]:
    """Flatten the pages ``gh api --paginate --slurp`` produced into objects.

    Returns
    -------
    list[collections.abc.Mapping[str, object]]
        Every object the pages hold, in listing order.

    Raises
    ------
    DraftAssetError
        If the decoded value is not the array-of-arrays shape ``--slurp``
        produces, or any page holds something other than objects.

    Notes
    -----
    An empty listing is not refused here: no draft matching the tag and a
    release with no attachments are different problems, so each caller decides
    after flattening.

    Examples
    --------
    >>> _entries([[{"id": 1}], [{"id": 2}]], what="release")
    [{'id': 1}, {'id': 2}]
    """
    match pages:
        case list() as page_list:
            pass
        case _:
            msg = f"the {what} listing is not the array of pages --slurp produces"
            raise DraftAssetError(msg)
    entries: list[cabc.Mapping[str, object]] = []
    for page in page_list:
        match page:
            case list() as entry_list:
                pass
            case _:
                msg = f"one page of the {what} listing is not an array"
                raise DraftAssetError(msg)
        for entry in entry_list:
            match entry:
                case dict() as mapping:
                    entries.append(mapping)
                case _:
                    msg = f"one entry in the {what} listing is not an object"
                    raise DraftAssetError(msg)
    return entries


def _draft_identifier(entry: cabc.Mapping[str, object], *, tag: str) -> int:
    """Return the identifier of a draft release or refuse the entry.

    Returns
    -------
    int
        The release identifier ``gh api`` needs for the asset listing.

    Raises
    ------
    ReleaseSelectionError
        If the entry carries no integer ``id``.
    """
    identifier = entry.get("id")
    if not _is_integer(identifier):
        msg = f"the draft release tagged {tag!r} carries no integer id: {identifier!r}"
        raise ReleaseSelectionError(msg)
    return typ.cast("int", identifier)


def _partition_drafts(
    matching: list[cabc.Mapping[str, object]],
) -> tuple[list[cabc.Mapping[str, object]], int]:
    """Split the tagged releases into the drafts and a count of the rest.

    Returns
    -------
    tuple[list[collections.abc.Mapping[str, object]], int]
        The draft entries in listing order, and the count of the rest.
    """
    drafts: list[cabc.Mapping[str, object]] = []
    published = 0
    for entry in matching:
        if entry.get("draft") is True:
            drafts.append(entry)
        else:
            published += 1
    return drafts, published


def select_draft_release(pages: object, *, tag: str) -> int:
    """Return the identifier of the one draft release carrying ``tag``.

    Parameters
    ----------
    pages
        Decoded ``gh api --method GET --paginate --slurp`` release pages. The
        list endpoint is used because the single-release endpoint refuses
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
        If ``tag`` does not identify exactly one draft with an integer id.

    Examples
    --------
    >>> pages = [[{"id": 7, "tag_name": "v1.2.3", "draft": True}]]
    >>> select_draft_release(pages, tag="v1.2.3")
    7
    >>> select_draft_release(pages, tag="v9.9.9")
    Traceback (most recent call last):
        ...
    release_draft_assets.ReleaseSelectionError: no draft release is tagged 'v9.9.9'
    """
    matching = [
        entry
        for entry in _entries(pages, what="release")
        if entry.get("tag_name") == tag
    ]
    drafts, published = _partition_drafts(matching)
    if published:
        msg = (
            f"the release tagged {tag!r} is published, not a draft, so there is "
            "no draft inventory to verify"
        )
        raise ReleaseSelectionError(msg)
    if not drafts:
        msg = f"no draft release is tagged {tag!r}"
        raise ReleaseSelectionError(msg)
    if len(drafts) > 1:
        msg = f"{len(drafts)} draft releases are tagged {tag!r}, not one"
        raise ReleaseSelectionError(msg)
    return _draft_identifier(drafts[0], tag=tag)


def _asset_name(entry: cabc.Mapping[str, object]) -> str:
    """Return one asset entry's name or refuse the entry.

    Returns
    -------
    str
        The attachment name, as the download plan will use it.

    Raises
    ------
    AssetListingError
        If the entry carries no string ``name``.
    """
    match entry.get("name"):
        case str() as name:
            return name
        case other:
            msg = f"a release asset entry carries no string name: {other!r}"
            raise AssetListingError(msg)


def _require_safe_names(names: list[str]) -> None:
    """Refuse names that are not bare file names inside the download directory.

    Raises
    ------
    AssetListingError
        If a name could resolve outside the download directory, or is not a
        bare file name.

    Notes
    -----
    Escaping is reported before malformed, as in
    :func:`release_inventory.validate_inventory`: traversal is the diagnosis.
    """
    escaping, malformed = release_asset_names.rejected_names(names)
    if escaping:
        msg = (
            "release asset names could resolve outside the download directory: "
            + ", ".join(repr(name) for name in escaping)
        )
        raise AssetListingError(msg)
    if malformed:
        msg = "release asset names are not bare file names: " + ", ".join(
            repr(name) for name in malformed
        )
        raise AssetListingError(msg)


def _require_plan_safe(names: list[str]) -> None:
    """Refuse names the tab-separated download plan cannot represent.

    Raises
    ------
    AssetListingError
        If any name contains a tab, a line feed, or a carriage return.

    Notes
    -----
    A line feed would forge an extra ``id<TAB>name`` record, so the download
    step would fetch an asset the listing never planned; a tab or carriage
    return would truncate the name it writes. Both are refused, not escaped.
    """
    unrepresentable = [name for name in names if set(name) & set(PLAN_DELIMITERS)]
    if unrepresentable:
        msg = (
            "release asset names cannot be represented in the download plan: "
            + ", ".join(repr(name) for name in unrepresentable)
        )
        raise AssetListingError(msg)


def _require_unique(names: list[str]) -> None:
    """Refuse two assets claiming one attachment name.

    Raises
    ------
    AssetListingError
        If any name appears more than once.

    Notes
    -----
    A duplicate would write the second asset over the first, so only one would
    ever be verified.
    """
    counts = collections.Counter(names)
    repeated = sorted(name for name, count in counts.items() if count > 1)
    if repeated:
        msg = "release asset names are duplicated: " + ", ".join(
            repr(name) for name in repeated
        )
        raise AssetListingError(msg)


def _entry_problems(entry: cabc.Mapping[str, object], name: str) -> list[str]:
    """Return every way one asset entry cannot be downloaded as claimed.

    Returns
    -------
    list[str]
        One diagnostic per defect, empty when the entry is usable.

    Notes
    -----
    Assets are addressed by number, and an attachment not finished storing, or
    one with no bytes, is not the artefact the trusted record describes.
    """
    identifier = entry.get("id")
    state = entry.get("state")
    size = entry.get("size")
    checks = (
        (_is_integer(identifier), f"{name!r} carries no integer id: {identifier!r}"),
        (state == UPLOADED_STATE, f"{name!r} is not uploaded (state {state!r})"),
        (
            _is_integer(size) and size != 0,
            f"{name!r} does not report a usable byte size: {size!r}",
        ),
    )
    return [message for holds, message in checks if not holds]


def _require_usable(entries: list[cabc.Mapping[str, object]], names: list[str]) -> None:
    """Refuse the listing if any entry cannot be downloaded as claimed.

    Raises
    ------
    AssetListingError
        If one or more entries report a defect, naming every one of them.
    """
    problems = [
        problem
        for entry, name in zip(entries, names, strict=True)
        for problem in _entry_problems(entry, name)
    ]
    if problems:
        msg = "the release asset listing is unusable: " + "; ".join(problems)
        raise AssetListingError(msg)


def plan_downloads(pages: object) -> list[tuple[int, str]]:
    """Return the ordered ``(asset_id, name)`` pairs to download.

    Parameters
    ----------
    pages
        Decoded ``gh api --method GET --paginate --slurp`` asset pages.

    Returns
    -------
    list[tuple[int, str]]
        Each asset's identifier and name, in listing order.

    Raises
    ------
    AssetListingError
        If the listing is empty, or any name or entry cannot be downloaded as
        claimed.

    Examples
    --------
    >>> pages = [[{"id": 11, "name": "a.tar.gz", "state": "uploaded",
    ...            "size": 5}]]
    >>> plan_downloads(pages)
    [(11, 'a.tar.gz')]
    """
    entries = _entries(pages, what="asset")
    if not entries:
        msg = "the release lists no assets, so there is nothing to verify"
        raise AssetListingError(msg)
    names = [_asset_name(entry) for entry in entries]
    _require_safe_names(names)
    _require_plan_safe(names)
    _require_unique(names)
    _require_usable(entries, names)
    return [
        (typ.cast("int", entry["id"]), name)
        for entry, name in zip(entries, names, strict=True)
    ]
