"""Read and write the trusted digest record at release boundaries.

This adapter owns JSON serialization and path errors for digest records. It
delegates the record shape to :mod:`release_digest_record`, so every consumer
uses the same validation rules after decoding.

Run via the declaration, inventory-verification, and finalization CLIs;
behavioural coverage lives in ``scripts/tests/test_release_digest_record_files.py``.
"""

import json
import typing as typ

import release_digest_record

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import pathlib


class UnreadableDigestRecordError(release_digest_record.DigestRecordError):
    """Report that a trusted record file could not be read."""


def read_digest_record(source: pathlib.Path) -> dict[str, str]:
    """Read JSON and validate the trusted digest mapping.

    Returns
    -------
    dict[str, str]
        Asset name to lowercase SHA-256 digest.

    Raises
    ------
    UnreadableDigestRecordError
        If the path cannot be read.
    release_digest_record.DigestRecordError
        If the file is not usable JSON or its value violates the record shape.

    Examples
    --------
    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     record = Path(tmp) / "record.json"
    ...     _ = record.write_text(
    ...         '{"assets": {"a": "' + "ab" * 32 + '"}}', encoding="utf-8"
    ...     )
    ...     read_digest_record(record)["a"] == "ab" * 32
    True
    """
    try:
        decoded = json.loads(source.read_bytes())
    except OSError as error:
        msg = f"the digest record {source} could not be read: {error}"
        raise UnreadableDigestRecordError(msg) from error
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        msg = f"the digest record {source} is not usable JSON text: {error}"
        raise release_digest_record.DigestRecordError(msg) from error
    return release_digest_record.validate_digest_record(decoded)


def write_digest_record(
    digests: cabc.Mapping[str, str], destination: pathlib.Path
) -> None:
    """Validate and write a deterministic digest record.

    The JSON is sorted, indented, and newline-terminated so identical input
    mappings always produce identical artefacts.

    Examples
    --------
    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     output = Path(tmp) / "record.json"
    ...     write_digest_record({"a": "ab" * 32}, output)
    ...     read_digest_record(output)["a"] == "ab" * 32
    True
    """
    validated = release_digest_record.validate_digest_record({
        release_digest_record.ASSETS_MEMBER: dict(digests)
    })
    serialized = json.dumps(
        {release_digest_record.ASSETS_MEMBER: validated}, indent=2, sort_keys=True
    )
    destination.write_text(serialized + "\n", encoding="utf-8")
