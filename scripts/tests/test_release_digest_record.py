"""Specify the trusted digest record's invariants and comparisons.

The record is permission to act on a release, so every refusal in
:mod:`scripts.release_digest_record` exists because the alternative is a
vacuous success and is proved here rather than left implied by the happy path.
The record is constructed directly, as a decoded adapter would construct it;
:mod:`scripts.tests.test_release_digest_record_files` covers the JSON shapes
that produce each refusal.
"""

import dataclasses
import operator
import string
import typing as typ

import pytest
import release_digest
import release_digest_record
from hypothesis import given, settings
from hypothesis import strategies as st

DIGEST = "ab" * 32
OTHER = "cd" * 32

#: An asset name a release could publish, kept short so examples stay readable.
_ASSET_NAME = st.from_regex(r"[a-z][a-z0-9_-]{0,7}", fullmatch=True)
#: A SHA-256 digest in either case, so normalization is exercised.
_HEX_DIGEST = st.text(
    alphabet=string.hexdigits,
    min_size=release_digest.DIGEST_HEX_LENGTH,
    max_size=release_digest.DIGEST_HEX_LENGTH,
)


@st.composite
def _declared_digests(draw: st.DrawFn) -> dict[str, str]:
    """Generate a non-empty asset mapping of arbitrary size and case.

    Returns
    -------
    dict[str, str]
        Distinct asset names mapped to hexadecimal digests in either case.
    """
    pairs = draw(
        st.lists(
            st.tuples(_ASSET_NAME, _HEX_DIGEST),
            min_size=1,
            max_size=6,
            unique_by=operator.itemgetter(0),
        )
    )
    return dict(pairs)


def _trusted(digests: dict[str, object]) -> release_digest_record.DigestRecord:
    """Build the record an adapter decodes from an external payload."""
    return release_digest_record.DigestRecord.from_mapping(digests)


def test_a_record_normalizes_digest_case() -> None:
    """Accept a digest written in either case, normalizing on the way in."""
    record = _trusted({"a": DIGEST.upper()})

    assert record.as_mapping() == {"a": DIGEST}, (
        "an uppercase digest describes the same bytes and must compare equal"
    )


@settings(max_examples=200, deadline=None, derandomize=True)
@given(declared=_declared_digests())
def test_from_mapping_normalizes_every_generated_record(
    declared: dict[str, str],
) -> None:
    """Normalize case and order for a non-empty mapping of any size.

    The comparisons rely on these invariants, so they have to hold for every
    size, case variant, and insertion order rather than for the few examples
    the table rows pin.
    """
    record = release_digest_record.DigestRecord.from_mapping(declared)
    expected = {name: digest.lower() for name, digest in declared.items()}

    assert record.as_mapping() == expected, "case must be normalized, not kept"
    assert record.names() == set(expected), "no asset name may be gained or lost"
    assert list(record.entries) == sorted(expected.items()), (
        "entries must be sorted by name, so insertion order cannot matter"
    )


@settings(max_examples=200, deadline=None, derandomize=True)
@given(declared=_declared_digests())
def test_a_canonical_record_is_the_same_however_it_was_built(
    declared: dict[str, str],
) -> None:
    """Agree between the factory and a canonical direct construction.

    An adapter decodes through the factory while a caller that already holds
    the canonical form may construct the record directly. The two must be
    interchangeable, or the type guarantee would depend on the constructor.
    """
    normalized = {name: digest.lower() for name, digest in declared.items()}
    from_factory = release_digest_record.DigestRecord.from_mapping(declared)
    canonical = release_digest_record.DigestRecord(tuple(sorted(normalized.items())))

    assert canonical == from_factory, (
        "a canonical direct construction must equal the factory's record"
    )
    assert canonical.as_mapping() == normalized, (
        "the direct record must expose the normalized mapping unchanged"
    )


@pytest.mark.parametrize(
    ("entries", "message"),
    [
        pytest.param((), "declares no assets", id="empty"),
        pytest.param(((1, DIGEST),), "as strings", id="non-string-name"),
        pytest.param((("a", DIGEST.upper()),), "lowercase", id="un-normalized"),
        pytest.param((("a", "nothex"),), "non-digest", id="not-hexadecimal"),
        pytest.param((("a", 1),), "as strings", id="non-string-digest"),
        pytest.param(
            (("a", DIGEST), ("a", OTHER)),
            "each asset name once",
            id="duplicate-name",
        ),
        pytest.param(
            (("b", DIGEST), ("a", DIGEST)),
            "in sorted order",
            id="unsorted-names",
        ),
    ],
)
def test_a_directly_constructed_record_is_validated(
    entries: tuple[tuple[object, object], ...], message: str
) -> None:
    """Refuse an invalid record however the caller built it.

    The comparison functions accept any record, so the invariant has to hold at
    construction rather than at the factory. These cases skip
    :meth:`DigestRecord.from_mapping` entirely.
    """
    malformed = typ.cast("tuple[tuple[str, str], ...]", entries)

    with pytest.raises(release_digest_record.DigestRecordError, match=message):
        _ = release_digest_record.DigestRecord(malformed)


def test_a_record_is_immutable() -> None:
    """Reject mutation, so a verified record cannot be rewritten in place.

    The assignment is written through ``setattr`` on purpose. A literal
    ``record.entries = ...`` is refused statically by the typechecker, so the
    only way to reach the runtime guard is to bypass its static twin.
    """
    record = _trusted({"a": DIGEST})
    rewritten = "entries"

    with pytest.raises(dataclasses.FrozenInstanceError) as raised:
        setattr(record, rewritten, ())
    assert str(raised.value) == "cannot assign to field 'entries'", (
        "the frozen error must name the field a caller tried to rewrite"
    )


def test_a_non_string_asset_name_is_refused_before_sorting() -> None:
    """Raise the record's own error, not ``TypeError``, for a bad key type.

    Sorting a mixed-type key set raises ``TypeError``, which callers do not
    catch. The contract is that a malformed mapping is a rejected record.
    """
    malformed = typ.cast("dict[str, object]", {"a": DIGEST, 1: DIGEST})

    with pytest.raises(release_digest_record.DigestRecordError, match="as strings"):
        _ = _trusted(malformed)


@pytest.mark.parametrize(
    ("declared", "message"),
    [
        pytest.param({}, "declares no assets", id="empty"),
        pytest.param({"a": 1}, "as strings", id="non-string-digest"),
        pytest.param({"a": "nothex"}, "non-digest", id="not-hex"),
        pytest.param({"a": "ab"}, "non-digest", id="too-short"),
        pytest.param({"a": DIGEST, "b": 2}, "as strings", id="one-bad-entry"),
    ],
)
def test_an_unusable_mapping_refuses_the_whole_record(
    declared: dict[str, object], message: str
) -> None:
    """Refuse the record, not merely the entry, so no asset is left unchecked.

    An empty mapping would verify nothing, a value the comparison could never
    match would silently pass an equality check, and a malformed digest must be
    diagnosed rather than compared as text.
    """
    with pytest.raises(release_digest_record.DigestRecordError, match=message):
        _ = _trusted(declared)


def test_a_covering_record_is_accepted() -> None:
    """Accept a record whose names are exactly the declaration's."""
    release_digest_record.require_record_matches_declaration(
        _trusted({"a": DIGEST}), expected=["a"]
    )


@pytest.mark.parametrize(
    ("digests", "expected", "message"),
    [
        pytest.param(
            {"a": DIGEST},
            ["a", "b"],
            "record omits",
            id="omits-declared",
        ),
        pytest.param(
            {"a": DIGEST, "b": OTHER},
            ["a"],
            "record adds",
            id="adds-undeclared",
        ),
        pytest.param(
            {"a": DIGEST},
            [],
            "names no asset",
            id="empty-declaration",
        ),
    ],
)
def test_a_record_that_does_not_cover_the_declaration_is_refused(
    digests: dict[str, object], expected: list[str], message: str
) -> None:
    """Refuse anything but exact coverage of the declared assets.

    Each row names the refusal it expects, because the cases differ in which
    side of the comparison is short: an omitted asset would go unverified, an
    undeclared asset has no declaration to verify against, and an empty
    declaration would pass a naive set-equality check over two empty sets.
    """
    record = _trusted(digests)

    with pytest.raises(release_digest_record.DigestRecordError, match=message):
        release_digest_record.require_record_matches_declaration(
            record, expected=expected
        )


def test_matching_downloads_are_accepted() -> None:
    """Accept downloads whose computed digests equal the recorded ones."""
    release_digest_record.require_downloads_match_record(
        {"a": DIGEST}, trusted=_trusted({"a": DIGEST})
    )


def test_a_differing_download_is_refused() -> None:
    """Refuse a download whose bytes differ from the recorded bytes."""
    with pytest.raises(release_digest_record.RecordedDigestMismatchError):
        release_digest_record.require_downloads_match_record(
            {"a": OTHER}, trusted=_trusted({"a": DIGEST})
        )


def test_a_missing_download_is_refused() -> None:
    """Refuse a recorded asset that never arrived.

    Iterating the downloads would miss this entirely, which is why the
    comparison runs over the trusted names.
    """
    with pytest.raises(release_digest_record.RecordedDigestMismatchError):
        release_digest_record.require_downloads_match_record(
            {}, trusted=_trusted({"a": DIGEST})
        )


def test_a_mismatch_names_every_wrong_asset() -> None:
    """Report all offenders in one message rather than only the first."""
    with pytest.raises(release_digest_record.RecordedDigestMismatchError) as caught:
        release_digest_record.require_downloads_match_record(
            {"a": OTHER, "b": "ef" * 32},
            trusted=_trusted({"a": DIGEST, "b": OTHER}),
        )

    assert "['a', 'b']" in str(caught.value), (
        f"both wrong assets must be named, got {caught.value}"
    )


def test_a_mismatch_is_catchable_through_the_digest_hierarchy() -> None:
    """Derive from both hierarchies, so a caller guarding either catches it."""
    digest_family = release_digest.DigestMismatchError
    mismatch = release_digest_record.RecordedDigestMismatchError

    assert issubclass(mismatch, digest_family), (
        "a caller guarding DigestMismatchError must not need a second clause"
    )
