"""Translate GitHub's release and asset pages into domain records.

``gh api --method GET --paginate --slurp`` answers with an array of pages, each
page an array of objects carrying GitHub's own field names. Those names are
provider detail: ``tag_name``, ``draft``, ``state``, and ``size`` are
meaningful to this adapter and to nothing downstream. Keeping the translation
here means a field renamed or an id that arrives as a float is one change in
one place, and :mod:`release_draft_assets` keeps deciding the same question.

The transcription is deliberately total: every field the domain records need is
type-checked before a record is built, so a listing this adapter accepts makes
the domain's rules total over well-typed values rather than defensive about
whatever JSON happened to arrive. Page shape and field types are refused here;
what a well-formed listing *means* — exactly one draft per tag, a stored
non-empty attachment, a safe name — stays in the domain.

Nothing here reaches the network or a file: the workflow step holds the token
and writes the listing, and
:mod:`release_draft_assets_cli` reads it. Behavioural coverage lives in
``scripts/tests/test_release_github_draft_listing.py``.
"""

import typing as typ

import release_draft_assets

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: The asset state GitHub reports for a fully uploaded attachment.
UPLOADED_STATE = "uploaded"


class GithubListingError(release_draft_assets.DraftAssetError):
    """Report a listing that is not the shape this adapter reads."""


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
    GithubListingError
        If the decoded value is not the array-of-arrays shape ``--slurp``
        produces, or any page holds something other than objects.

    Notes
    -----
    An empty listing is not refused here: no draft matching the tag and a
    release with no attachments are different problems, so the domain decides
    after translation.

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
            raise GithubListingError(msg)
    entries: list[cabc.Mapping[str, object]] = []
    for page in page_list:
        match page:
            case list() as entry_list:
                pass
            case _:
                msg = f"one page of the {what} listing is not an array"
                raise GithubListingError(msg)
        for entry in entry_list:
            match entry:
                case dict() as mapping:
                    entries.append(mapping)
                case _:
                    msg = f"one entry in the {what} listing is not an object"
                    raise GithubListingError(msg)
    return entries


def _integer_field(entry: cabc.Mapping[str, object], field: str, *, what: str) -> int:
    """Return one entry's integer field or refuse the entry.

    Returns
    -------
    int
        The field's value.

    Raises
    ------
    GithubListingError
        If the field is absent or is not an integer.
    """
    value = entry.get(field)
    if not _is_integer(value):
        msg = f"{what} entry carries no integer {field}: {value!r}"
        raise GithubListingError(msg)
    return typ.cast("int", value)


def _string_field(entry: cabc.Mapping[str, object], field: str, *, what: str) -> str:
    """Return one entry's string field or refuse the entry.

    Returns
    -------
    str
        The field's value.

    Raises
    ------
    GithubListingError
        If the field is absent or is not a string.
    """
    match entry.get(field):
        case str() as value:
            return value
        case other:
            msg = f"{what} entry carries no string {field}: {other!r}"
            raise GithubListingError(msg)


def _boolean_field(entry: cabc.Mapping[str, object], field: str, *, what: str) -> bool:
    """Return one entry's boolean field or refuse the entry.

    Returns
    -------
    bool
        The field's value.

    Raises
    ------
    GithubListingError
        If the field is absent or is not a boolean.

    Notes
    -----
    ``draft`` is transcribed rather than interpreted: a release whose flag is
    absent is not a listing this adapter understands, and guessing that it is
    published would report the wrong reason for refusing the tag.
    """
    match entry.get(field):
        case bool() as value:
            return value
        case other:
            msg = f"{what} entry carries no boolean {field}: {other!r}"
            raise GithubListingError(msg)


def releases_from_pages(pages: object) -> list[release_draft_assets.Release]:
    """Translate slurped release pages into domain release records.

    Parameters
    ----------
    pages
        Decoded ``gh api --method GET --paginate --slurp`` release pages, as
        the list endpoint reports them.

    Returns
    -------
    list[release_draft_assets.Release]
        One record per release, in listing order.

    Notes
    -----
    A listing that is not slurped pages, or a release carrying no integer
    ``id``, string ``tag_name``, or boolean ``draft``, is refused with
    :class:`GithubListingError` by the transcription this delegates to.

    Examples
    --------
    >>> pages = [[{"id": 7, "tag_name": "v1.2.3", "draft": True}]]
    >>> releases_from_pages(pages)
    [Release(identifier=7, tag='v1.2.3', is_draft=True)]
    """
    return [
        release_draft_assets.Release(
            identifier=_integer_field(entry, "id", what="release"),
            tag=_string_field(entry, "tag_name", what="release"),
            is_draft=_boolean_field(entry, "draft", what="release"),
        )
        for entry in _entries(pages, what="release")
    ]


def _asset_from_entry(entry: cabc.Mapping[str, object]) -> release_draft_assets.Asset:
    """Translate one asset entry into a domain asset record.

    Returns
    -------
    release_draft_assets.Asset
        The transcribed asset.

    Notes
    -----
    An entry carrying no integer ``id`` or ``size``, string ``name``, or string
    ``state`` is refused with :class:`GithubListingError` by the field readers
    this delegates to.
    """
    state = _string_field(entry, "state", what="release asset")
    return release_draft_assets.Asset(
        identifier=_integer_field(entry, "id", what="release asset"),
        name=_string_field(entry, "name", what="release asset"),
        is_stored=state == UPLOADED_STATE,
        size=_integer_field(entry, "size", what="release asset"),
    )


def assets_from_pages(pages: object) -> list[release_draft_assets.Asset]:
    """Translate slurped asset pages into domain asset records.

    Parameters
    ----------
    pages
        Decoded ``gh api --method GET --paginate --slurp`` asset pages, as one
        release's asset listing reports them.

    Returns
    -------
    list[release_draft_assets.Asset]
        One record per attachment, in listing order.

    Notes
    -----
    A listing that is not slurped pages, or an attachment carrying no integer
    ``id`` or ``size``, string ``name``, or string ``state``, is refused with
    :class:`GithubListingError` by the transcription this delegates to.

    Examples
    --------
    >>> pages = [[{"id": 11, "name": "a.tar.gz", "state": "uploaded",
    ...            "size": 5}]]
    >>> assets_from_pages(pages)
    [Asset(identifier=11, name='a.tar.gz', is_stored=True, size=5)]
    """
    return [_asset_from_entry(entry) for entry in _entries(pages, what="asset")]
