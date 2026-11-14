"""Specify the draft-release selection and download-planning rules.

Both rules decide from domain values, never from GitHub's JSON: translating a
listing is :mod:`release_github_draft_listing`'s work and is covered by its own
suite. Keeping the rules exercised over :class:`Release` and :class:`Asset`
values is what shows the decisions do not depend on a provider field name. The
generated cases extend the duplicate-name and unsafe-name rows across a wide
input space because both refusals protect the directory the trusted inputs sit
beside.
"""

import pytest
from hypothesis import given, settings
from hypothesis import strategies as st
from release_draft_assets import (
    Asset,
    AssetListingError,
    Release,
    ReleaseSelectionError,
    plan_downloads,
    select_draft_release,
)

#: Asset names built only from characters no path flavour treats specially.
SAFE_NAMES = st.from_regex(r"[A-Za-z0-9][A-Za-z0-9._-]{0,39}", fullmatch=True)

TAG = "v0.1.0"


def _release(identifier: int, *, tag: str = TAG, is_draft: bool = True) -> Release:
    """Return one release record as the adapter would translate it."""
    return Release(identifier=identifier, tag=tag, is_draft=is_draft)


def _asset(
    identifier: int,
    name: str,
    *,
    is_stored: bool = True,
    size: int = 5,
) -> Asset:
    """Return one asset record as the adapter would translate it."""
    return Asset(identifier=identifier, name=name, is_stored=is_stored, size=size)


def test_the_matching_draft_is_selected_among_other_releases() -> None:
    """Select by tag and draft state, not merely by position in the listing."""
    releases = [_release(1, tag="v0.0.9", is_draft=False), _release(7)]

    assert select_draft_release(releases, tag=TAG) == 7, (
        "the one draft carrying the tag must be selected"
    )


def test_a_release_with_another_tag_is_not_selected() -> None:
    """Select by tag, not merely by being the only draft in the listing."""
    with pytest.raises(ReleaseSelectionError, match="no draft release"):
        select_draft_release([_release(7, tag="v0.0.9")], tag=TAG)


def test_a_published_release_with_the_tag_is_refused() -> None:
    """Report publication, which means the draft this job was to verify is gone."""
    with pytest.raises(ReleaseSelectionError, match="published, not a draft"):
        select_draft_release([_release(7, is_draft=False)], tag=TAG)


def test_two_drafts_with_the_tag_are_refused() -> None:
    """Refuse an ambiguous listing rather than pick one draft arbitrarily."""
    with pytest.raises(ReleaseSelectionError, match="2 draft releases"):
        select_draft_release([_release(7), _release(8)], tag=TAG)


def test_the_download_plan_preserves_listing_order() -> None:
    """Keep the order given, so a rerun downloads the same bytes in the same order."""
    assets = [_asset(11, "a.tar.gz"), _asset(12, "b.deb")]

    assert plan_downloads(assets) == [
        _asset(11, "a.tar.gz"),
        _asset(12, "b.deb"),
    ], "the plan must preserve the order it was given"


def test_an_empty_asset_listing_is_refused() -> None:
    """Refuse a release with nothing to verify instead of verifying nothing."""
    with pytest.raises(AssetListingError, match="lists no assets"):
        _ = plan_downloads([])


@given(name=SAFE_NAMES)
@settings(max_examples=50)
def test_a_duplicated_asset_name_is_refused(name: str) -> None:
    """Refuse a duplicate, which would let one download overwrite another."""
    with pytest.raises(AssetListingError, match="duplicated") as caught:
        _ = plan_downloads([_asset(11, name), _asset(12, name)])
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
        _ = plan_downloads([_asset(11, name)])


@given(suffix=st.text(max_size=20))
@settings(max_examples=50)
def test_a_traversing_name_is_refused_whatever_it_appends(suffix: str) -> None:
    """Reject a leading ``..`` component no matter what follows it."""
    with pytest.raises(AssetListingError, match="outside the download directory"):
        _ = plan_downloads([_asset(11, f"../{suffix}")])


@pytest.mark.parametrize("name", ["sub/netsuke", "sub\\netsuke", "", ".", ".."])
def test_a_name_that_is_not_a_bare_file_name_is_refused(name: str) -> None:
    """Refuse nested, empty, and reserved spellings the downloader cannot use."""
    with pytest.raises(AssetListingError):
        _ = plan_downloads([_asset(11, name)])


def test_an_asset_that_is_not_stored_is_refused() -> None:
    """Refuse an attachment GitHub has not finished storing."""
    with pytest.raises(AssetListingError, match="not an uploaded attachment"):
        _ = plan_downloads([_asset(11, "a.tar.gz", is_stored=False)])


@pytest.mark.parametrize("size", [0, -1])
def test_an_asset_without_a_usable_byte_size_is_refused(size: int) -> None:
    """Refuse an empty or nonsensical attachment, which would hash wrongly."""
    with pytest.raises(AssetListingError, match="byte size"):
        _ = plan_downloads([_asset(11, "a.tar.gz", size=size)])


def test_every_defect_in_one_listing_is_reported() -> None:
    """Describe the whole listing, so one run explains every unusable asset."""
    assets = [
        _asset(11, "a.tar.gz", is_stored=False, size=0),
        _asset(12, "b.deb", size=0),
    ]

    with pytest.raises(AssetListingError) as caught:
        _ = plan_downloads(assets)
    message = str(caught.value)
    assert "'a.tar.gz'" in message, message
    assert "'b.deb'" in message, message


def test_the_plan_is_a_list_of_asset_records() -> None:
    """Pin the shape the plan renderer reads, one record per downloaded asset."""
    plan: list[Asset] = plan_downloads([_asset(11, "a.tar.gz")])

    assert plan == [_asset(11, "a.tar.gz")], "the plan must be a list of asset records"
