"""Specify the draft-release selection and download-planning rules.

Both rules decide from a listing the workflow saved, never from the network, so
they can be exercised over the exact shapes GitHub returns: several pages, a
published release carrying the same tag, and asset entries that are not the
uploaded, non-empty attachments a verification needs. The generated cases
extend the duplicate-name and unsafe-name rows across a wide input space
because both refusals protect the directory the trusted inputs sit beside.
"""

import pytest
from hypothesis import given, settings
from hypothesis import strategies as st
from release_draft_assets import (
    AssetListingError,
    DraftAssetError,
    ReleaseSelectionError,
    plan_downloads,
    select_draft_release,
)

#: Asset names built only from characters no path flavour treats specially.
SAFE_NAMES = st.from_regex(r"[A-Za-z0-9][A-Za-z0-9._-]{0,39}", fullmatch=True)

TAG = "v0.1.0"


def _release(
    identifier: object, *, tag: str = TAG, draft: bool = True
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


def test_the_matching_draft_is_selected_across_pages() -> None:
    """Flatten the pages before matching, so pagination cannot hide the draft."""
    pages = [
        [_release(1, tag="v0.0.9", draft=False)],
        [_release(7)],
    ]

    assert select_draft_release(pages, tag=TAG) == 7, (
        "the draft on the second page must be selected"
    )


def test_a_release_with_another_tag_is_not_selected() -> None:
    """Select by tag, not merely by being the only draft in the listing."""
    with pytest.raises(ReleaseSelectionError, match="no draft release"):
        select_draft_release([[_release(7, tag="v0.0.9")]], tag=TAG)


def test_a_published_release_with_the_tag_is_refused() -> None:
    """Report publication, which means the draft this job was to verify is gone."""
    with pytest.raises(ReleaseSelectionError, match="published, not a draft"):
        select_draft_release([[_release(7, draft=False)]], tag=TAG)


def test_two_drafts_with_the_tag_are_refused() -> None:
    """Refuse an ambiguous listing rather than pick one draft arbitrarily."""
    pages = [[_release(7), _release(8)]]

    with pytest.raises(ReleaseSelectionError, match="2 draft releases"):
        select_draft_release(pages, tag=TAG)


@pytest.mark.parametrize("identifier", [None, "7", 7.0, True])
def test_a_draft_without_an_integer_id_is_refused(identifier: object) -> None:
    """Require the identifier the asset listing is addressed by."""
    with pytest.raises(ReleaseSelectionError, match="no integer id"):
        select_draft_release([[_release(identifier)]], tag=TAG)


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
        select_draft_release(pages, tag=TAG)


def test_the_download_plan_preserves_listing_order() -> None:
    """Keep GitHub's order, so a rerun downloads the same bytes in the same order."""
    pages = [[_asset(11, "a.tar.gz"), _asset(12, "b.deb")]]

    assert plan_downloads(pages) == [(11, "a.tar.gz"), (12, "b.deb")], (
        "the plan must preserve the listing order"
    )


def test_an_empty_asset_listing_is_refused() -> None:
    """Refuse a release with nothing to verify instead of verifying nothing."""
    with pytest.raises(AssetListingError, match="lists no assets"):
        _ = plan_downloads([[]])


@given(name=SAFE_NAMES)
@settings(max_examples=50)
def test_a_duplicated_asset_name_is_refused(name: str) -> None:
    """Refuse a duplicate, which would let one download overwrite another."""
    pages = [[_asset(11, name), _asset(12, name)]]

    with pytest.raises(AssetListingError, match="duplicated") as caught:
        _ = plan_downloads(pages)
    assert name in str(caught.value), "the diagnostic must name the duplicate"


@pytest.mark.parametrize(
    "name",
    ["../escape.tar.gz", "/absolute.tar.gz", "C:\\windows.tar.gz", ".."],
)
def test_a_name_that_could_escape_the_download_directory_is_refused(
    name: str,
) -> None:
    """Refuse traversal before judging mere malformation, as inventory does."""
    with pytest.raises(AssetListingError, match="outside the download directory"):
        _ = plan_downloads([[_asset(11, name)]])


@given(suffix=st.text(max_size=20))
@settings(max_examples=50)
def test_a_traversing_name_is_refused_whatever_it_appends(suffix: str) -> None:
    """Reject a leading ``..`` component no matter what follows it."""
    with pytest.raises(AssetListingError, match="outside the download directory"):
        _ = plan_downloads([[_asset(11, f"../{suffix}")]])


@pytest.mark.parametrize("name", ["sub/netsuke", "sub\\netsuke", "", ".", ".."])
def test_a_name_that_is_not_a_bare_file_name_is_refused(name: str) -> None:
    """Refuse nested, empty, and reserved spellings the downloader cannot use."""
    with pytest.raises(AssetListingError):
        _ = plan_downloads([[_asset(11, name)]])


@pytest.mark.parametrize("identifier", [None, "11", 11.5, True, 11.0])
def test_an_asset_without_an_integer_id_is_refused(identifier: object) -> None:
    """Require the identifier the download endpoint addresses."""
    with pytest.raises(AssetListingError, match="no integer id"):
        _ = plan_downloads([[_asset(identifier, "a.tar.gz")]])


@pytest.mark.parametrize("state", ["new", "starter", None, "UPLOADED"])
def test_an_asset_that_is_not_uploaded_is_refused(state: object) -> None:
    """Refuse an attachment GitHub has not finished storing."""
    with pytest.raises(AssetListingError, match="not uploaded"):
        _ = plan_downloads([[_asset(11, "a.tar.gz", state=state)]])


@pytest.mark.parametrize("size", [0, None, "5", 5.0, True])
def test_an_asset_without_a_usable_byte_size_is_refused(size: object) -> None:
    """Refuse an empty or unmeasurable attachment, which would hash wrongly."""
    with pytest.raises(AssetListingError, match="byte size"):
        _ = plan_downloads([[_asset(11, "a.tar.gz", size=size)]])


@pytest.mark.parametrize("name", [None, 11, ["a.tar.gz"]])
def test_an_asset_without_a_string_name_is_refused(name: object) -> None:
    """Refuse an entry the plan could not turn into a file name."""
    with pytest.raises(AssetListingError, match="no string name"):
        _ = plan_downloads([[_asset(11, name)]])


def test_every_defect_in_one_listing_is_reported() -> None:
    """Describe the whole listing, so one run explains every unusable asset."""
    pages = [
        [
            _asset(11, "a.tar.gz", state="new", size=0),
            _asset(12, "b.deb", size=0),
        ]
    ]

    with pytest.raises(AssetListingError) as caught:
        _ = plan_downloads(pages)
    message = str(caught.value)
    assert "'a.tar.gz'" in message, message
    assert "'b.deb'" in message, message


def test_the_plan_type_is_a_list_of_pairs() -> None:
    """Pin the shape the workflow step reads, one pair per downloaded asset."""
    plan: list[tuple[int, str]] = plan_downloads([[_asset(11, "a.tar.gz")]])

    assert list(plan) == [(11, "a.tar.gz")], (
        "the plan must be a list of identifier and name pairs"
    )
