"""Exercise the trusted digest record's reader, writer and comparisons.

Every refusal in :mod:`scripts.release_digest_record` exists because the
alternative is a vacuous success, so each one is proved here rather than left
implied by the happy path. The record is permission to act on a release; a test
that only showed a well-formed record being accepted would not say what happens
when one is not.
"""

import json
import typing as typ

import pytest
import release_digest
from release_digest_record import (
    DigestRecordError,
    RecordedDigestMismatchError,
    require_downloads_match_record,
    require_record_matches_declaration,
)
from release_digest_record_files import (
    UnreadableDigestRecordError,
    read_digest_record,
    write_digest_record,
)

if typ.TYPE_CHECKING:
    import pathlib

DIGEST = "ab" * 32
OTHER = "cd" * 32


def _record(destination: pathlib.Path, digests: dict[str, str]) -> pathlib.Path:
    """Write a well-formed record."""
    write_digest_record(digests, destination)
    return destination


def test_a_written_record_round_trips(tmp_path: pathlib.Path) -> None:
    """Read back exactly what was written."""
    source = _record(tmp_path / "record.json", {"a": DIGEST, "b": OTHER})

    assert read_digest_record(source) == {"a": DIGEST, "b": OTHER}, (
        "the reader must return the mapping the writer produced"
    )


def test_the_record_is_written_in_sorted_order(tmp_path: pathlib.Path) -> None:
    """Write a stable file, so two runs over one tree are byte-identical."""
    source = _record(tmp_path / "record.json", {"b": OTHER, "a": DIGEST})

    assert (
        source.read_text(encoding="utf-8")
        == json.dumps({"assets": {"a": DIGEST, "b": OTHER}}, indent=2, sort_keys=True)
        + "\n"
    ), "the record must be sorted and newline-terminated"


def test_digests_are_lowercased_on_read(tmp_path: pathlib.Path) -> None:
    """Accept a record written in either case, normalising on the way in."""
    source = tmp_path / "record.json"
    upper = {"assets": {"a": DIGEST.upper()}}
    _ = source.write_text(json.dumps(upper), encoding="utf-8")

    assert read_digest_record(source) == {"a": DIGEST}, (
        "an uppercase digest describes the same bytes and must compare equal"
    )


@pytest.mark.parametrize(
    ("payload", "reason"),
    [
        ("not json at all", "a file that is not JSON must be refused"),
        ("[]", "a JSON array is not a record"),
        ('{"assets": null}', "an absent mapping is not an empty one"),
        ('{"assets": []}', "a list is not a name-to-digest mapping"),
        ('{"assets": {}}', "an empty record would verify nothing"),
        ('{"assets": {"a": 1}}', "a digest must be a string"),
        ('{"assets": {"a": "nothex"}}', "a non-digest must be refused, not compared"),
        ('{"assets": {"a": "ab"}}', "a short digest must be refused"),
        (
            '{"assets": {"a": "' + "ab" * 32 + '", "b": 2}}',
            "one bad entry refuses the whole record",
        ),
    ],
)
def test_a_malformed_record_is_refused(
    tmp_path: pathlib.Path, payload: str, reason: str
) -> None:
    """Refuse every shape that cannot support a comparison, with a reason."""
    source = tmp_path / "record.json"
    _ = source.write_text(payload, encoding="utf-8")

    with pytest.raises(DigestRecordError):
        _ = read_digest_record(source)


def test_an_unreadable_record_is_refused(tmp_path: pathlib.Path) -> None:
    """Refuse a path that does not exist rather than treating it as empty.

    The narrower type is asserted, not merely the base one. A consumer that
    reports this as a rejected release rather than an unreadable input sends an
    operator to the upload for a fault in a path, so which subclass is raised
    is part of the contract rather than an implementation detail.
    """
    with pytest.raises(UnreadableDigestRecordError):
        _ = read_digest_record(tmp_path / "absent.json")


def test_a_covering_record_is_accepted() -> None:
    """Accept a record whose names are exactly the declaration's."""
    require_record_matches_declaration({"a": DIGEST}, expected=["a"])


def test_a_record_that_omits_a_declared_asset_is_refused() -> None:
    """Refuse a record that would leave an asset uncompared."""
    with pytest.raises(DigestRecordError):
        require_record_matches_declaration({"a": DIGEST}, expected=["a", "b"])


def test_a_record_that_adds_an_undeclared_asset_is_refused() -> None:
    """Refuse a record naming an asset the release never declared."""
    with pytest.raises(DigestRecordError):
        require_record_matches_declaration({"a": DIGEST, "b": OTHER}, expected=["a"])


def test_an_empty_declaration_is_refused() -> None:
    """Refuse the vacuous case, where set equality holds over two empty sets."""
    with pytest.raises(DigestRecordError):
        require_record_matches_declaration({}, expected=[])


def test_matching_downloads_are_accepted() -> None:
    """Accept downloads whose computed digests equal the recorded ones."""
    require_downloads_match_record({"a": DIGEST}, trusted={"a": DIGEST})


def test_a_differing_download_is_refused() -> None:
    """Refuse a download whose bytes differ from the recorded bytes."""
    with pytest.raises(RecordedDigestMismatchError):
        require_downloads_match_record({"a": OTHER}, trusted={"a": DIGEST})


def test_a_missing_download_is_refused() -> None:
    """Refuse a recorded asset that never arrived.

    Iterating the downloads would miss this entirely, which is why the
    comparison runs over the trusted names.
    """
    with pytest.raises(RecordedDigestMismatchError):
        require_downloads_match_record({}, trusted={"a": DIGEST})


def test_a_mismatch_names_every_wrong_asset() -> None:
    """Report all offenders in one message rather than only the first."""
    with pytest.raises(RecordedDigestMismatchError) as caught:
        require_downloads_match_record(
            {"a": OTHER, "b": "ef" * 32}, trusted={"a": DIGEST, "b": OTHER}
        )

    assert "['a', 'b']" in str(caught.value), (
        f"both wrong assets must be named, got {caught.value}"
    )


def test_a_mismatch_is_catchable_through_the_digest_hierarchy() -> None:
    """Derive from both hierarchies, so a caller guarding either catches it."""
    digest_family = release_digest.DigestMismatchError
    assert issubclass(RecordedDigestMismatchError, digest_family), (
        "a caller guarding DigestMismatchError must not need a second clause"
    )
