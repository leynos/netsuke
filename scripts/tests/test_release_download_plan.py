"""Specify the text format the download plan is recorded in.

The workflow step reads the plan line by line and splits each line on a tab, so
the format has one job beyond rendering: refuse a name that could forge or
truncate a record. These cases pin the rendered shape and every delimiter that
would break it, including the forged second record a line feed would produce.
"""

from itertools import starmap

import pytest
from hypothesis import given, settings
from hypothesis import strategies as st
from release_download_plan import PlanFormatError, format_plan
from release_draft_assets import Asset, DraftAssetError

#: Asset names built only from characters no path flavour treats specially.
SAFE_NAMES = st.from_regex(r"[A-Za-z0-9][A-Za-z0-9._-]{0,39}", fullmatch=True)


def _asset(identifier: int, name: str) -> Asset:
    """Return one stored, non-empty asset record."""
    return Asset(identifier=identifier, name=name, is_stored=True, size=5)


def test_the_plan_is_rendered_in_the_order_given() -> None:
    """Keep the plan's order, which is the listing's order, not a sorted one."""
    plan = [_asset(11, "a.tar.gz"), _asset(12, "b.deb")]

    assert format_plan(plan) == "11\ta.tar.gz\n12\tb.deb\n", (
        "one id-and-name record per asset, in the order given"
    )


def test_an_empty_plan_renders_as_no_records() -> None:
    """Render nothing for nothing; refusing an empty listing is the domain's job."""
    assert not format_plan([]), "an empty plan must carry no records"


@pytest.mark.parametrize("name", ["a\tb.tar.gz", "a\nb.tar.gz", "a\rb.tar.gz"])
def test_a_name_the_plan_cannot_carry_is_refused(name: str) -> None:
    """Refuse a delimiter that would truncate or split the name it writes."""
    with pytest.raises(PlanFormatError, match="download plan") as caught:
        _ = format_plan([_asset(11, name)])
    assert repr(name) in str(caught.value), "the diagnostic must name the name"


def test_a_line_feed_could_forge_an_extra_record() -> None:
    r"""Refuse the name that would plan a download the listing never carried.

    The plan is read line by line and split on tabs, so ``"11\ta\n99\tb"``
    would render two records: the asset the listing named, and asset 99, which
    it did not.
    """
    with pytest.raises(PlanFormatError, match="download plan"):
        _ = format_plan([_asset(11, "11\ta\n99\tb")])


def test_the_format_refusal_is_a_policy_rejection() -> None:
    """Fail through the shared error, so the command keeps one rejection status."""
    assert issubclass(PlanFormatError, DraftAssetError), (
        "a format refusal must be reported as a listing rejection"
    )


@given(names=st.lists(SAFE_NAMES, min_size=1, max_size=6, unique=True))
@settings(max_examples=50)
def test_a_safe_name_set_always_round_trips_through_the_format(
    names: list[str],
) -> None:
    """Render and re-read generated plans, so the format never loses a record."""
    plan = list(starmap(_asset, enumerate(names, start=1)))

    records = [line.split("\t") for line in format_plan(plan).splitlines() if line]

    assert records == [[str(asset.identifier), asset.name] for asset in plan], (
        "every planned asset must survive the round trip intact"
    )
