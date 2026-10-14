"""Specify Windows MSI release-rank parsing without workflow dependencies."""

import pytest
from hypothesis import given
from hypothesis import strategies as st
from windows_msi_release_rank import FINAL_RELEASE_RANK, ReleaseRankError, release_rank


@pytest.mark.parametrize(
    ("version", "expected"),
    [
        ("1.2.3-beta1", 1),
        ("1.2.3-beta2", 2),
        ("1.2.3-beta65534", 65_534),
        ("1.2.3", FINAL_RELEASE_RANK),
    ],
)
def test_supported_release_versions_have_expected_ranks(
    version: str, expected: int
) -> None:
    """Assign each documented release form its Windows Installer rank."""
    assert release_rank(version) == expected, f"{version} should map to {expected}"


@pytest.mark.parametrize(
    "version",
    [
        "1.2.3-beta0",
        "1.2.3-beta65535",
        "1.2.3-beta65536",
        "1.2.3-beta",
        "1.2.3-betax",
        "1.2-beta1",
        "1.2.3.4-beta1",
        "01.2.3-beta1",
        "1.2.3-alpha1",
        "1.2.3-rc1",
    ],
)
def test_unsupported_release_versions_fail(version: str) -> None:
    """Reject unorderable beta sequences and non-contract version forms."""
    with pytest.raises(ReleaseRankError):
        release_rank(version)


def test_every_supported_beta_rank_precedes_the_matching_final_release() -> None:
    """Exhaust the MSI rank domain so every permitted beta remains ordered."""
    final_rank = release_rank("1.2.3")
    for rank in range(1, FINAL_RELEASE_RANK):
        assert release_rank(f"1.2.3-beta{rank}") == rank, (
            f"beta{rank} should retain its sequence as the MSI rank"
        )
        assert rank < final_rank, f"beta{rank} should precede the final release"


@given(
    major=st.integers(min_value=0, max_value=9_999),
    minor=st.integers(min_value=0, max_value=9_999),
    patch=st.integers(min_value=0, max_value=9_999),
    rank=st.integers(min_value=1, max_value=FINAL_RELEASE_RANK - 1),
)
def test_valid_beta_versions_map_below_final_releases(
    major: int, minor: int, patch: int, rank: int
) -> None:
    """Keep valid beta ordering below the matching generated final release."""
    numeric_version = f"{major}.{minor}.{patch}"
    assert release_rank(f"{numeric_version}-beta{rank}") == rank, (
        "the beta rank should equal its sequence"
    )
    assert rank < release_rank(numeric_version), (
        "every valid beta rank should precede the matching final release"
    )


@given(
    sequence=st.one_of(
        st.just("0"),
        st.integers(min_value=FINAL_RELEASE_RANK, max_value=2**64).map(str),
        st.sampled_from(("", "x", "01", "-1")),
    )
)
def test_invalid_beta_sequences_fail(sequence: str) -> None:
    """Reject every generated class of disallowed beta sequence."""
    with pytest.raises(ReleaseRankError):
        release_rank(f"1.2.3-beta{sequence}")
