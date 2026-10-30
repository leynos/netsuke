"""Specify the trusted digest record's invariants and comparisons.

The record is permission to act on a release, so every refusal in
:mod:`scripts.release_digest_record` exists because the alternative is a
vacuous success and is proved here rather than left implied by the happy path.
The record is constructed directly, as a decoded adapter would construct it;
:mod:`scripts.tests.test_release_digest_record_files` covers the JSON shapes
that produce each refusal.
"""

import typing as typ

import pytest
import release_digest
import release_digest_record

DIGEST = "ab" * 32
OTHER = "cd" * 32


def _trusted(digests: dict[str, object]) -> release_digest_record.DigestRecord:
    """Build the record an adapter decodes from an external payload."""
    return release_digest_record.DigestRecord.from_mapping(digests)


def test_a_record_normalizes_digest_case() -> None:
    """Accept a digest written in either case, normalizing on the way in."""
    record = _trusted({"a": DIGEST.upper()})

    assert record.as_mapping() == {"a": DIGEST}, (
        "an uppercase digest describes the same bytes and must compare equal"
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


def test_a_non_string_asset_name_is_refused_before_sorting() -> None:
    """Raise the record's own error, not ``TypeError``, for a bad key type.

    Sorting a mixed-type key set raises ``TypeError``, which callers do not
    catch. The contract is that a malformed mapping is a rejected record.
    """
    malformed = typ.cast("dict[str, object]", {1: DIGEST})

    with pytest.raises(release_digest_record.DigestRecordError, match="as strings"):
        _ = _trusted(malformed)


def test_an_empty_record_is_refused() -> None:
    """Refuse a record that would verify nothing."""
    with pytest.raises(release_digest_record.DigestRecordError):
        _ = _trusted({})


def test_a_digest_that_is_not_a_string_is_refused() -> None:
    """Refuse a value the comparison could never match."""
    with pytest.raises(release_digest_record.DigestRecordError):
        _ = _trusted({"a": 1})


@pytest.mark.parametrize("digest", ["nothex", "ab"], ids=["not-hex", "too-short"])
def test_a_digest_that_is_not_a_digest_is_refused(digest: str) -> None:
    """Refuse a malformed digest rather than comparing it as text."""
    with pytest.raises(release_digest_record.DigestRecordError):
        _ = _trusted({"a": digest})


def test_one_bad_entry_refuses_the_whole_record() -> None:
    """Refuse the record, not merely the entry, so no asset is left unchecked."""
    with pytest.raises(release_digest_record.DigestRecordError):
        _ = _trusted({"a": DIGEST, "b": 2})


def test_a_covering_record_is_accepted() -> None:
    """Accept a record whose names are exactly the declaration's."""
    release_digest_record.require_record_matches_declaration(
        _trusted({"a": DIGEST}), expected=["a"]
    )


def test_a_record_that_omits_a_declared_asset_is_refused() -> None:
    """Refuse a record that would leave an asset uncompared."""
    with pytest.raises(release_digest_record.DigestRecordError):
        release_digest_record.require_record_matches_declaration(
            _trusted({"a": DIGEST}), expected=["a", "b"]
        )


def test_a_record_that_adds_an_undeclared_asset_is_refused() -> None:
    """Refuse a record naming an asset the release never declared."""
    with pytest.raises(release_digest_record.DigestRecordError):
        release_digest_record.require_record_matches_declaration(
            _trusted({"a": DIGEST, "b": OTHER}), expected=["a"]
        )


def test_an_empty_declaration_is_refused() -> None:
    """Refuse the vacuous case, where set equality holds over two empty sets."""
    with pytest.raises(release_digest_record.DigestRecordError):
        release_digest_record.require_record_matches_declaration(
            _trusted({"a": DIGEST}), expected=[]
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
