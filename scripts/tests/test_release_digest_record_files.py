"""Test JSON and filesystem adaptation for digest records."""

import json
import typing as typ

import pytest
import release_digest_record
from release_digest_record_files import (
    UnreadableDigestRecordError,
    read_digest_record,
    write_digest_record,
)

if typ.TYPE_CHECKING:
    import pathlib


DIGEST = "ab" * 32
OTHER = "cd" * 32


def test_record_file_round_trips_a_valid_mapping(tmp_path: pathlib.Path) -> None:
    """Serialize and parse the record through the adapter boundary."""
    path = tmp_path / "record.json"
    write_digest_record({"a": DIGEST}, path)

    assert read_digest_record(path).as_mapping() == {"a": DIGEST}, (
        "the JSON adapter must round-trip the validated digest mapping"
    )
    assert path.read_text(encoding="utf-8").endswith("\n"), (
        "the serialized record must end with a newline"
    )


def test_the_record_is_written_in_sorted_order(tmp_path: pathlib.Path) -> None:
    """Write a stable file, so two runs over one tree are byte-identical."""
    path = tmp_path / "record.json"
    write_digest_record({"b": OTHER, "a": DIGEST}, path)

    assert (
        path.read_text(encoding="utf-8")
        == json.dumps({"assets": {"a": DIGEST, "b": OTHER}}, indent=2, sort_keys=True)
        + "\n"
    ), "the record must be sorted and newline-terminated"


def test_unreadable_path_uses_the_adapter_error(tmp_path: pathlib.Path) -> None:
    """Distinguish a missing file from invalid record contents."""
    with pytest.raises(UnreadableDigestRecordError):
        read_digest_record(tmp_path / "missing.json")


def test_json_shape_errors_are_policy_errors_not_adapter_errors(
    tmp_path: pathlib.Path,
) -> None:
    """Decode the member here and leave value validation to the record.

    The two error families send an operator to different places: an unusable
    path is a wiring fault, while a record that parses but declares the wrong
    shape is a rejected release. The adapter must not blur them.
    """
    path = tmp_path / "record.json"
    _ = path.write_text("[]", encoding="utf-8")

    with pytest.raises(release_digest_record.DigestRecordError) as caught:
        read_digest_record(path)

    assert not isinstance(caught.value, UnreadableDigestRecordError), (
        "a parsed payload is a policy rejection, not an unreadable input"
    )


@pytest.mark.parametrize(
    ("payload", "fragment"),
    [
        ("not json at all", "not usable JSON text"),
        ("[]", "must be a mapping"),
        ('{"assets": null}', "must carry an 'assets' mapping"),
        ('{"assets": []}', "must carry an 'assets' mapping"),
        ('{"assets": {}}', "declares no assets"),
        ('{"assets": {"a": 1}}', "map asset names to digests as strings"),
        ('{"assets": {"a": "nothex"}}', "holds a non-digest"),
        ('{"assets": {"a": "ab"}}', "holds a non-digest"),
        (
            '{"assets": {"a": "' + DIGEST + '", "b": 2}}',
            "map asset names to digests as strings",
        ),
    ],
    ids=[
        "not-json",
        "json-array",
        "assets-null",
        "assets-list",
        "assets-empty",
        "digest-is-an-integer",
        "digest-not-hex",
        "digest-too-short",
        "one-bad-entry",
    ],
)
def test_a_malformed_record_is_refused(
    tmp_path: pathlib.Path, payload: str, fragment: str
) -> None:
    """Refuse every decoded shape that cannot support a comparison."""
    path = tmp_path / "record.json"
    _ = path.write_text(payload, encoding="utf-8")

    with pytest.raises(release_digest_record.DigestRecordError, match=fragment):
        read_digest_record(path)
