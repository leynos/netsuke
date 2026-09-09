"""The per-test tier, asserted against this repository's own file.

Split from ``timeout_ordering_test`` so neither module outgrows the
400-line limit the lint gate enforces.
"""

import pytest
from nextest_budgets import bounds_a_single_test
from timeout_budgets import NEXTEST_CONFIG


@pytest.fixture(scope="module")
def nextest_config() -> str:
    """Return the nextest configuration file's text.

    Returns
    -------
    str
        The file's contents.
    """
    return NEXTEST_CONFIG.read_text(encoding="utf-8")


def test_the_default_profile_bounds_a_test_it_matches_no_override_for(
    nextest_config: str,
) -> None:
    """An override bounds its filter's tests; the profile bounds the rest.

    ``largest_test_allowance`` reports the largest budget anywhere in
    the file, so deleting the profile's own ``slow-timeout`` and leaving
    the Windows override behind still reports 600 s while every test the
    override does not match runs with no bound at all. That is the state
    this assertion exists to detect, and nothing else here would.

    Proved by mutation: commenting out ``[profile.default]``'s own
    ``slow-timeout`` fails this test and nothing else.
    """
    assert bounds_a_single_test(nextest_config), (
        "[profile.default] itself must set slow-timeout with terminate-after; "
        "an override satisfies the file as a whole while leaving every test it "
        "does not match unbounded"
    )
