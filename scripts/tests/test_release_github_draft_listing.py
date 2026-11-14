"""Specify how slurped GitHub pages become domain release and asset records.

The adapter owns the one place GitHub's field names and JSON types are read, so
these cases pin both halves of its contract: every field the domain needs is
type-checked before a record is built, and the flattened listing keeps the order
the pages arrived in. The generated cases split a unique-name listing across
arbitrary page boundaries, because pagination is exactly where an order bug or
a duplicate that straddles two pages would hide.
"""

import itertools

import pytest
from hypothesis import given, settings
from hypothesis import strategies as st
from release_draft_assets import (
    Asset,
    AssetListingError,
    DraftAssetError,
    Release,
    plan_downloads,
)
from release_github_draft_listing import (
    GithubListingError,
    assets_from_pages,
    releases_from_pages,
)

#: Asset names built only from characters no path flavour treats specially.
SAFE_NAMES = st.from_regex(r"[A-Za-z0-9][A-Za-z0-9._-]{0,39}", fullmatch=True)

TAG = "v0.1.0"


def _release(
    identifier: object, *, tag: object = TAG, draft: object = True
) -> dict[str, object]:
    """Return one release entry as the list endpoint reports it."""
    return {"id": identifier, "tag_name": tag, "draft": draft}


def _asset(
    identifier: object,
    name: object,
    *,
    state: object = "uploaded",
    size: object = 5,
) -> dict[str, object]:
    """Return one asset entry as the release's asset listing reports it."""
    return {"id": identifier, "name": name, "state": state, "size": size}


def _split(
    entries: list[Asset], boundaries: list[int]
) -> list[list[dict[str, object]]]:
    """Split translated entries into pages at the given boundaries.

    Returns
    -------
    list[list[dict[str, object]]]
        One page of asset entries per interval between boundaries.

    Notes
    -----
    Boundaries may repeat, which produces an empty page: a slurped listing can
    hold one, and it must not disturb the flattened order.
    """
    edges = [0, *boundaries, len(entries)]
    return [
        [_asset(entry.identifier, entry.name) for entry in entries[start:stop]]
        for start, stop in itertools.pairwise(edges)
    ]


def _release_record(
    identifier: int, *, tag: str = TAG, is_draft: bool = True
) -> Release:
    """Return the domain record the adapter should build for one release."""
    return Release(identifier=identifier, tag=tag, is_draft=is_draft)


def test_releases_are_translated_in_listing_order() -> None:
    """Transcribe every release the pages hold, in order, across pagination."""
    pages = [
        [_release(1, tag="v0.0.9", draft=False)],
        [_release(7)],
    ]

    assert releases_from_pages(pages) == [
        _release_record(1, tag="v0.0.9", is_draft=False),
        _release_record(7),
    ], "every release must be transcribed, in listing order"


@pytest.mark.parametrize("identifier", [None, "7", 7.0, True])
def test_a_release_without_an_integer_id_is_refused(identifier: object) -> None:
    """Require the identifier the asset listing is addressed by."""
    with pytest.raises(GithubListingError, match="no integer id"):
        _ = releases_from_pages([[_release(identifier)]])


@pytest.mark.parametrize("tag", [None, 7, ["v0.1.0"]])
def test_a_release_without_a_string_tag_is_refused(tag: object) -> None:
    """Refuse a release whose tag cannot be compared with the requested one."""
    with pytest.raises(GithubListingError, match="no string tag_name"):
        _ = releases_from_pages([[_release(7, tag=tag)]])


@pytest.mark.parametrize("draft", [None, "true", 1, 0])
def test_a_release_without_a_boolean_draft_flag_is_refused(draft: object) -> None:
    """Refuse a release whose draft state would have to be guessed."""
    with pytest.raises(GithubListingError, match="no boolean draft"):
        _ = releases_from_pages([[_release(7, draft=draft)]])


@pytest.mark.parametrize("identifier", [None, "11", 11.5, True, 11.0])
def test_an_asset_without_an_integer_id_is_refused(identifier: object) -> None:
    """Require the identifier the download endpoint addresses."""
    with pytest.raises(GithubListingError, match="no integer id"):
        _ = assets_from_pages([[_asset(identifier, "a.tar.gz")]])


@pytest.mark.parametrize("name", [None, 11, ["a.tar.gz"]])
def test_an_asset_without_a_string_name_is_refused(name: object) -> None:
    """Refuse an entry the plan could not turn into a file name."""
    with pytest.raises(GithubListingError, match="no string name"):
        _ = assets_from_pages([[_asset(11, name)]])


@pytest.mark.parametrize("state", [None, 1, ["uploaded"]])
def test_an_asset_without_a_string_state_is_refused(state: object) -> None:
    """Refuse an entry whose upload state cannot be read."""
    with pytest.raises(GithubListingError, match="no string state"):
        _ = assets_from_pages([[_asset(11, "a.tar.gz", state=state)]])


@pytest.mark.parametrize("size", [None, "5", 5.0, True])
def test_an_asset_without_an_integer_size_is_refused(size: object) -> None:
    """Require the byte count the verification compares against."""
    with pytest.raises(GithubListingError, match="no integer size"):
        _ = assets_from_pages([[_asset(11, "a.tar.gz", size=size)]])


@pytest.mark.parametrize(
    "pages",
    [
        pytest.param({"id": 7}, id="not-an-array-of-pages"),
        pytest.param([{"id": 7}], id="page-is-not-an-array"),
        pytest.param([[7]], id="entry-is-not-an-object"),
    ],
)
def test_a_listing_that_is_not_slurped_pages_is_refused(pages: object) -> None:
    """Refuse a shape ``gh api --slurp`` never produces."""
    with pytest.raises(DraftAssetError):
        _ = releases_from_pages(pages)


def test_the_uploaded_state_is_translated_to_a_stored_attachment() -> None:
    """Read GitHub's state vocabulary once, so the domain need not know it."""
    assets = assets_from_pages([
        [_asset(11, "a.tar.gz"), _asset(12, "b.deb", state="new")]
    ])

    assert [asset.is_stored for asset in assets] == [True, False], (
        "only the uploaded state means the attachment is stored"
    )


@pytest.mark.parametrize("state", ["new", "starter", "UPLOADED"])
def test_an_unstored_attachment_is_refused_through_the_domain(state: str) -> None:
    """Keep the refusal in the domain while the vocabulary stays in the adapter."""
    pages = [[_asset(11, "a.tar.gz", state=state)]]

    with pytest.raises(AssetListingError, match="not an uploaded attachment"):
        _ = plan_downloads(assets_from_pages(pages))


@given(
    names=st.lists(SAFE_NAMES, min_size=1, max_size=6, unique=True),
    data=st.data(),
)
@settings(max_examples=50)
def test_the_plan_preserves_order_across_generated_pages(
    names: list[str], data: st.DataObject
) -> None:
    """Flatten pages before planning, so pagination cannot reorder a download."""
    boundaries = sorted(
        data.draw(
            st.lists(
                st.integers(min_value=0, max_value=len(names)),
                max_size=3,
            )
        )
    )
    entries = [
        Asset(identifier=identifier, name=name, is_stored=True, size=5)
        for identifier, name in enumerate(names, start=1)
    ]

    plan = plan_downloads(assets_from_pages(_split(entries, boundaries)))

    assert plan == entries, "the plan must follow the flattened listing order"


@given(name=SAFE_NAMES)
@settings(max_examples=50)
def test_a_duplicate_spanning_two_pages_is_refused(name: str) -> None:
    """Reject a duplicate even when an empty page sits between its halves.

    Notes
    -----
    Pagination is a presentation detail of the listing, so a name repeated
    across a page boundary is the same overwrite hazard as one repeated within
    a page.
    """
    pages = [[_asset(11, name)], [], [_asset(12, name)]]

    with pytest.raises(AssetListingError, match="duplicated") as caught:
        _ = plan_downloads(assets_from_pages(pages))
    assert name in str(caught.value), "the diagnostic must name the duplicate"
