"""Specify the semantic-version grammar and the prerelease predicate.

The grammar is a closed predicate over strings, so it is pinned by named rows
rather than by the identity module's properties. Keeping them here matters:
while these rows lived only in ``test_release_candidate``, the row labelled
``a-leading-zero-in-the-core`` was refused by the tag/version agreement rather
than by the leading-zero rule its name claimed, so an implementation that
permitted leading zeros still passed. Each reject row below therefore names
the clause that must refuse it, and the accept rows make the predicate's
truth table two-sided rather than a list of things it happens to reject.

The accept rows include ``0.1.0-beta04`` deliberately. It reads like the
archetypal leading-zero violation and is nothing of the sort: the padded
identifier carries a letter, so it is alphanumeric, and Cargo's `semver` crate
accepts it. A row asserting the opposite was written here first and failed,
which is the whole reason this corpus is checked against the reference
implementation rather than against intuition.
"""

import pytest
from release_semver import is_prerelease, is_valid_semver

_TAG = "v0.1.0-beta4"


@pytest.mark.parametrize(
    "version",
    [
        pytest.param("0.1.0", id="a-plain-release"),
        pytest.param("0.1.0-beta4", id="an-alphanumeric-prerelease"),
        pytest.param("0.1.0-0", id="the-zero-prerelease-identifier"),
        pytest.param("0.1.0-beta.4", id="a-dotted-prerelease"),
        pytest.param("0.1.0-x.7.z.92", id="a-mixed-multi-part-prerelease"),
        pytest.param("0.1.0+meta", id="build-metadata"),
        pytest.param("0.1.0-beta4+meta.sha.5114f85", id="both-fields-together"),
        pytest.param("10.20.30", id="multi-digit-core-identifiers"),
        pytest.param("0.0.0", id="the-zero-version"),
        pytest.param("1.0.0-alpha-a.b-c-somethinglong", id="a-long-hyphenated-one"),
        pytest.param("0.1.0-beta04", id="padding-inside-an-alphanumeric-part"),
        pytest.param(
            "0.1.0-x7.z.92", id="a-part-that-is-neither-pure-digit-nor-letter"
        ),
    ],
)
def test_well_formed_versions_are_accepted(version: str) -> None:
    """Accept each well-formed shape, clause by clause."""
    assert is_valid_semver(version), f"{version!r} is not a well-formed version"


@pytest.mark.parametrize(
    ("version", "reason"),
    [
        pytest.param("01.0.0", "a leading zero in the major", id="major-leading-zero"),
        pytest.param("0.01.0", "a leading zero in the minor", id="minor-leading-zero"),
        pytest.param("0.1.00", "a leading zero in the patch", id="patch-leading-zero"),
        pytest.param(
            "0.1.0-01", "a leading zero in a numeric prerelease", id="prerelease"
        ),
        pytest.param(
            "0.1.0-beta.01",
            "a leading zero in a numeric prerelease part",
            id="dotted-prerelease",
        ),
        pytest.param(
            "0.1.0-1.01",
            "a leading zero after a well-formed first part",
            id="leading-zero-after-a-good-part",
        ),
        pytest.param("0.1", "fewer than three core identifiers", id="two-part-core"),
        pytest.param(
            "0.1.0.0", "more than three core identifiers", id="four-part-core"
        ),
        pytest.param("0.1.0-", "an empty prerelease", id="empty-prerelease"),
        pytest.param("0.1.0+", "an empty metadata field", id="empty-metadata"),
        pytest.param("0.1.0-beta..4", "an empty prerelease part", id="double-dot"),
        pytest.param("0.1.0 ", "trailing whitespace", id="trailing-space"),
        pytest.param(" 0.1.0", "leading whitespace", id="leading-space"),
        pytest.param("v0.1.0", "a tag prefix", id="v-prefix"),
        pytest.param(
            "0.1.0_beta", "an underscore where a hyphen belongs", id="underscore"
        ),
        pytest.param(
            "0.1.0-beta_4", "an underscore in a prerelease part", id="in-prerelease"
        ),
        pytest.param("0.1.0-beta 4", "an embedded space", id="embedded-space"),
        pytest.param("", "an empty string", id="empty"),
        pytest.param(
            "1.2.3-", "an empty prerelease on a full core", id="trailing-hyphen"
        ),
    ],
)
def test_malformed_versions_are_rejected(version: str, reason: str) -> None:
    """Refuse each malformed shape, naming the clause that refuses it."""
    assert not is_valid_semver(version), f"{version!r} must be refused: {reason}"


@pytest.mark.parametrize(
    ("version", "expected"),
    [
        pytest.param("0.1.0-beta4", True, id="an-alphanumeric-prerelease"),
        pytest.param("0.1.0-rc.1", True, id="a-dotted-prerelease"),
        pytest.param("0.1.0-0", True, id="a-numeric-prerelease"),
        pytest.param("0.1.0", False, id="a-plain-release"),
        pytest.param("0.1.0+meta", False, id="metadata-is-not-a-prerelease"),
        pytest.param("0.1.0-beta4+meta", True, id="metadata-beside-a-prerelease"),
    ],
)
def test_the_prerelease_predicate_reads_the_prerelease_field(
    version: str,
    *,
    expected: bool,
) -> None:
    """Read the prerelease field, never the presence of any suffix.

    The middle rows are what separate this from "the version has a suffix":
    build metadata alone does not make a version a prerelease, so a predicate
    testing for any `-` or `+` would report `0.1.0+meta` as one. Its
    counterpart is the last row, which carries both fields: reading the
    prerelease group reports it as a prerelease even though metadata is
    present, where a predicate testing "has metadata" would not.
    """
    assert is_prerelease(version) is expected, (
        "the predicate must read the prerelease field, not any suffix"
    )


def test_the_predicate_agrees_with_the_primary_tag_on_a_known_pair() -> None:
    """Anchor the grammar to the tag this release path was written for."""
    assert is_valid_semver(_TAG[1:]), f"{_TAG[1:]} is a well-formed version"
    assert is_prerelease(_TAG[1:]), f"{_TAG[1:]} is a prerelease"
