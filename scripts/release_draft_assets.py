"""Decide which draft release to verify and which of its assets to download.

The release workflow has to find the draft it just populated and fetch every
attachment back before it can compare those bytes with the trusted record. The
network calls belong to the workflow step that holds the token; this module
decides, from the already-parsed JSON ``gh api --slurp`` produced, whether that
listing can support the decision at all.

Two refusals are deliberately stricter than GitHub's own behaviour. Selecting a
release requires exactly one *draft* with the requested tag, because a published
release with that tag means the draft this job was asked to verify is gone, and
two drafts mean the listing does not identify one release. Planning downloads
requires every asset to be an uploaded, non-empty regular attachment with a
bare, safe name that the tab-separated download plan can carry, because a name
that could escape the download directory would place attacker-chosen bytes
beside the trusted inputs, and a zero-byte asset would be hashed as an empty
file rather than as the artefact it claims to be.

Nothing here parses JSON, touches a file, or reaches the network:
:mod:`release_draft_assets_cli` owns those effects. The name judgement is
:mod:`release_asset_names`, shared with inventory verification, so the download
plan and the later inventory check agree on what a publishable name is.

Run via ``scripts/release_draft_assets_cli.py``; behavioural coverage lives in
``scripts/tests/test_release_draft_assets.py``.
"""

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

    ``bool`` is a subclass of ``int`` in Python, so an unguarded check would
    accept ``True`` as an asset identifier and hand ``gh`` a nonsense argument.

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

    Parameters
    ----------
    pages
        Decoded JSON, expected to be an array of page arrays of objects.
    what
        Noun naming the listing in any diagnostic, such as ``"release"``.

    Returns
    -------
    list[dict[str, object]]
        Every object the pages hold, in listing order.

    Raises
    ------
    DraftAssetError
        If the decoded value is not the array-of-arrays shape ``--slurp``
        produces, or any page holds something other than objects.

    Notes
    -----
    A listing that is merely empty is *not* refused here. Emptiness means
    different things to the two callers -- no draft matches the tag versus a
    release with no attachments -- so each decides it after flattening.

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
        If no draft carries ``tag``, a release carrying it is already
        published, more than one draft carries it, or the chosen draft has no
        integer identifier.

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
    published = [entry for entry in matching if entry.get("draft") is not True]
    if published:
        msg = (
            f"the release tagged {tag!r} is published, not a draft, so there is "
            "no draft inventory to verify"
        )
        raise ReleaseSelectionError(msg)
    drafts = [entry for entry in matching if entry.get("draft") is True]
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
    Escaping is reported before malformed, matching
    :func:`release_inventory.validate_inventory`: a traversal attempt is the
    security diagnosis, and naming it as mere malformation would bury it.
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
    A bare file name may still hold a delimiter. A line feed would forge an
    extra ``id<TAB>name`` record, so the download step would fetch an asset the
    listing never planned; a tab or carriage return would truncate or corrupt
    the name the step writes. Both are refused here rather than escaped,
    because an asset name is published data and nothing legitimate needs one.
    """
    unrepresentable = [
        name for name in names if any(char in name for char in PLAN_DELIMITERS)
    ]
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
    A duplicated name would make the download plan write the second asset over
    the first, so only one of the two would ever be verified.
    """
    counts: dict[str, int] = {}
    for name in names:
        counts[name] = counts.get(name, 0) + 1
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
    An identifier is required because the download endpoint addresses assets by
    number, not by name. The state and size are required because an attachment
    GitHub has not finished storing, or one with no bytes, is not the artefact
    the trusted record describes; hashing it would compare the wrong thing.
    """
    problems: list[str] = []
    if not _is_integer(entry.get("id")):
        problems.append(f"{name!r} carries no integer id: {entry.get('id')!r}")
    state = entry.get("state")
    if state != UPLOADED_STATE:
        problems.append(f"{name!r} is not uploaded (state {state!r})")
    size = entry.get("size")
    if not _is_integer(size) or size == 0:
        problems.append(f"{name!r} does not report a usable byte size: {size!r}")
    return problems


def plan_downloads(pages: object) -> list[tuple[int, str]]:
    """Return the ordered ``(asset_id, name)`` pairs to download.

    Parameters
    ----------
    pages
        Decoded ``gh api --method GET --paginate --slurp`` asset pages for one
        release.

    Returns
    -------
    list[tuple[int, str]]
        Each asset's identifier and name, in listing order.

    Raises
    ------
    AssetListingError
        If the listing is empty, names an unsafe, malformed, or
        plan-unrepresentable attachment, repeats a name, or holds an entry
        without an integer identifier, a non-``uploaded`` state, or a usable
        byte size.

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
    problems = [
        problem
        for entry, name in zip(entries, names, strict=True)
        for problem in _entry_problems(entry, name)
    ]
    if problems:
        msg = "the release asset listing is unusable: " + "; ".join(problems)
        raise AssetListingError(msg)
    return [
        (typ.cast("int", entry["id"]), name)
        for entry, name in zip(entries, names, strict=True)
    ]
