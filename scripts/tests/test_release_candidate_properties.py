"""Check the release-candidate resolver's SemVer ordering against generated tags.

The resolver chooses which Netsuke commit the downstream canaries build, and
its ``auto`` rule depends on ``latest_version_tag`` returning the highest
version tag. The properties here generate version tags in the shapes that
``VERSION_TAG`` accepts and check the ordering that ``version_key`` induces.

The ordering properties draw SemVer-canonical pre-release identifiers, meaning
numeric identifiers carry no leading zero, because that is the form SemVer 2.0
defines precedence for. ``VERSION_TAG`` also accepts leading zeros. The strict
expected failure at the end of this module records what that acceptance does
to the key: it makes distinct tags compare equal.
"""

import importlib.util
import sys
from pathlib import Path

from hypothesis import given, settings
from hypothesis import strategies as st

SCRIPT = (
    Path(__file__).resolve().parents[2]
    / ".github"
    / "scripts"
    / "resolve_release_candidate.py"
)
_SPEC = importlib.util.spec_from_file_location("resolve_release_candidate", SCRIPT)
if _SPEC is None or _SPEC.loader is None:
    message = f"cannot load {SCRIPT}"
    raise ImportError(message)
resolver = importlib.util.module_from_spec(_SPEC)
sys.modules[_SPEC.name] = resolver
_SPEC.loader.exec_module(resolver)

#: Small core numbers keep the generated tags readable in failure reports.
CORE_NUMBER = st.integers(min_value=0, max_value=999)
#: A numeric pre-release identifier, spelled without a leading zero.
NUMERIC_IDENTIFIER = st.integers(min_value=0, max_value=999).map(str)
#: An alphanumeric identifier. The mandatory non-digit in the first or second
#: position keeps it out of the numeric form, whose ordering rules differ.
ALPHANUMERIC_IDENTIFIER = st.from_regex(
    r"[0-9]{0,2}[A-Za-z-][0-9A-Za-z-]{0,3}", fullmatch=True
)
PRERELEASE = st.lists(
    st.one_of(NUMERIC_IDENTIFIER, ALPHANUMERIC_IDENTIFIER),
    min_size=1,
    max_size=3,
).map(".".join)


def _spell_tag(major: int, minor: int, patch: int, prerelease: str | None) -> str:
    """Spell a version tag from its core numbers and optional pre-release."""
    core = f"v{major}.{minor}.{patch}"
    return core if prerelease is None else f"{core}-{prerelease}"


#: Version tags that ``version_key`` must treat as version tags.
VERSION_TAGS = st.builds(
    _spell_tag,
    CORE_NUMBER,
    CORE_NUMBER,
    CORE_NUMBER,
    st.one_of(st.none(), PRERELEASE),
)
#: Tag names that are not version tags and must be ignored by the resolver.
NON_VERSION_TAG = st.sampled_from(["vTEST", "v1.2", "main", "v01.0.0", "1.0.0"])


def _key(tag: str) -> resolver.VersionKey:
    """Return the precedence key of a generated tag, which must be a version tag."""
    key = resolver.version_key(tag)
    assert key is not None, f"generated tag {tag!r} must be a version tag"
    return key


@settings(max_examples=200, deadline=None, derandomize=True)
@given(left=VERSION_TAGS, right=VERSION_TAGS)
def test_version_order_is_total(left: str, right: str) -> None:
    """Every pair of version tags is ordered one way or the other."""
    left_key, right_key = _key(left), _key(right)
    assert left_key <= right_key or right_key <= left_key, (
        f"{left!r} and {right!r} must be comparable under version_key"
    )


@settings(max_examples=200, deadline=None, derandomize=True)
@given(tags=st.lists(VERSION_TAGS, min_size=3, max_size=3))
def test_version_order_is_transitive(tags: list[str]) -> None:
    """Three version tags in a chain by key satisfy the transitive ordering."""
    # Sorting makes the premise hold for every example, so no input is wasted.
    first, second, third = sorted(tags, key=_key)
    assert _key(first) <= _key(second) <= _key(third), (
        f"sorted tags {first!r}, {second!r}, {third!r} must form a chain"
    )
    assert _key(first) <= _key(third), (
        f"{first!r} <= {second!r} <= {third!r} must imply {first!r} <= {third!r}"
    )


@settings(max_examples=200, deadline=None, derandomize=True)
@given(left=VERSION_TAGS, right=VERSION_TAGS)
def test_equal_keys_only_arise_from_equal_tags(left: str, right: str) -> None:
    """Two canonical version tags with equal keys are the same tag."""
    if _key(left) == _key(right):
        assert left == right, (
            f"{left!r} and {right!r} share a version_key but are different tags"
        )


@settings(max_examples=200, deadline=None, derandomize=True)
@given(
    tags=st.lists(VERSION_TAGS, min_size=1, max_size=8),
    noise=st.lists(NON_VERSION_TAG, max_size=3),
)
def test_latest_version_tag_has_maximal_key(tags: list[str], noise: list[str]) -> None:
    """The tag returned for a non-empty set has a key no other version tag exceeds."""
    candidates = [*tags, *noise]
    latest = resolver.latest_version_tag(candidates)
    assert latest is not None, "a set with a version tag must have a latest tag"
    assert latest in candidates, f"latest tag {latest!r} must come from the input"
    latest_key = _key(latest)
    for tag in tags:
        assert _key(tag) <= latest_key, (
            f"{tag!r} must not outrank the latest tag {latest!r}"
        )


@settings(max_examples=100, deadline=None, derandomize=True)
@given(tags=st.lists(NON_VERSION_TAG, max_size=5))
def test_latest_version_tag_is_none_without_version_tags(
    tags: list[str],
) -> None:
    """A set holding no version tag yields no latest tag."""
    assert resolver.latest_version_tag(tags) is None, (
        f"non-version tags {tags!r} must not yield a latest tag"
    )


@settings(max_examples=50, deadline=None, derandomize=True)
@given(number=st.integers(min_value=0, max_value=99))
def test_leading_zero_identifier_is_not_a_version_tag(number: int) -> None:
    """A zero-padded numeric pre-release identifier is not SemVer, so not a tag."""
    assert resolver.version_key(f"v1.0.0-rc.{number}") is not None, (
        f"rc.{number} must be a version tag"
    )
    assert resolver.version_key(f"v1.0.0-rc.0{number}") is None, (
        f"rc.0{number} has a leading zero and must not be read as a version"
    )
