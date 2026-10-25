"""Test JSON and filesystem adaptation for digest records."""

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


def test_record_file_round_trips_a_valid_mapping(tmp_path: pathlib.Path) -> None:
    """Serialize and parse the record through the adapter boundary."""
    path = tmp_path / "record.json"
    write_digest_record({"a": DIGEST}, path)

    assert read_digest_record(path) == {"a": DIGEST}, (
        "the JSON adapter must round-trip the validated digest mapping"
    )
    assert path.read_text(encoding="utf-8").endswith("\n"), (
        "the serialized record must end with a newline"
    )


def test_unreadable_path_uses_the_adapter_error(tmp_path: pathlib.Path) -> None:
    """Distinguish a missing file from invalid record contents."""
    with pytest.raises(UnreadableDigestRecordError):
        read_digest_record(tmp_path / "missing.json")


def test_json_shape_errors_remain_policy_errors(tmp_path: pathlib.Path) -> None:
    """Keep parsed-value validation in the pure policy module."""
    path = tmp_path / "record.json"
    _ = path.write_text("[]", encoding="utf-8")

    with pytest.raises(release_digest_record.DigestRecordError):
        read_digest_record(path)
