"""Read and write the trusted digest record at release boundaries.

This adapter owns JSON serialization, the record's ``assets`` member, and path
errors for digest records. It delegates entry validation to
:class:`release_digest_record.DigestRecord`, so every consumer receives the
same validated record after decoding.

Run via the declaration, inventory-verification, and finalization CLIs;
behavioural coverage lives in ``scripts/tests/test_release_digest_record_files.py``.
"""

import json
import typing as typ

import release_digest_record

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import pathlib

#: The JSON member holding the name-to-digest mapping.
ASSETS_MEMBER = "assets"


class UnreadableDigestRecordError(release_digest_record.DigestRecordError):
    """Report that a trusted record file could not be read."""


def read_digest_record(
    source: pathlib.Path,
) -> release_digest_record.DigestRecord:
    """Read JSON and decode the trusted digest record.

    Returns
    -------
    release_digest_record.DigestRecord
        The validated record with lowercase digests.

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
    ...     read_digest_record(record).as_mapping()["a"] == "ab" * 32
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
    return validate_digest_record(decoded)


def validate_digest_record(
    record: object,
) -> release_digest_record.DigestRecord:
    """Decode a record's ``assets`` member into a validated digest record.

    Parameters
    ----------
    record
        Decoded JSON value.

    Returns
    -------
    release_digest_record.DigestRecord
        The validated record with lowercase digests.

    Notes
    -----
    The value invariant belongs to
    :meth:`release_digest_record.DigestRecord.from_mapping`, whose
    ``DigestRecordError`` propagates unchanged. What stays here is the member
    name and the shape of the value that carries it.

    Examples
    --------
    >>> record = validate_digest_record({"assets": {"a": "AB" * 32}})
    >>> record.as_mapping()["a"] == "ab" * 32
    True
    >>> validate_digest_record({"assets": {}})  # doctest: +ELLIPSIS
    Traceback (most recent call last):
        ...
    release_digest_record.DigestRecordError: the digest record declares no assets...
    """
    return release_digest_record.DigestRecord.from_mapping(_asset_mapping(record))


def _asset_mapping(record: object) -> dict[str, object]:
    """Return the non-empty asset mapping from a decoded record.

    Returns
    -------
    dict[str, object]
        The unvalidated digest values from the record's ``assets`` member,
        keyed by asset name.

    Raises
    ------
    release_digest_record.DigestRecordError
        If the record is not a non-empty mapping of string-keyed asset
        entries.

    Examples
    --------
    >>> _asset_mapping({"assets": {"a": "ab"}})
    {'a': 'ab'}
    """
    if not isinstance(record, dict):
        msg = f"the digest record must be a mapping, got {type(record).__name__}"
        raise release_digest_record.DigestRecordError(msg)
    declared = record.get(ASSETS_MEMBER)
    if not isinstance(declared, dict):
        msg = (
            f"the digest record must carry an '{ASSETS_MEMBER}' mapping, got "
            f"{type(declared).__name__}"
        )
        raise release_digest_record.DigestRecordError(msg)
    if not declared:
        msg = "the digest record declares no assets, so there is nothing to verify"
        raise release_digest_record.DigestRecordError(msg)
    if not all(isinstance(name, str) for name in declared):
        msg = "the digest record must map asset names to digests as strings"
        raise release_digest_record.DigestRecordError(msg)
    return typ.cast("dict[str, object]", declared)


def write_digest_record(
    digests: cabc.Mapping[str, str], destination: pathlib.Path
) -> None:
    """Validate and write a deterministic digest record.

    The JSON is sorted, indented, and newline-terminated so identical input
    mappings always produce identical artefacts.

    Notes
    -----
    The mapping is validated by building a
    :class:`release_digest_record.DigestRecord`, so a writer cannot emit a
    record its own reader would refuse.

    Examples
    --------
    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     output = Path(tmp) / "record.json"
    ...     write_digest_record({"a": "ab" * 32}, output)
    ...     read_digest_record(output).as_mapping()["a"] == "ab" * 32
    True
    """
    validated = release_digest_record.DigestRecord.from_mapping(digests)
    serialized = json.dumps(
        {ASSETS_MEMBER: validated.as_mapping()}, indent=2, sort_keys=True
    )
    destination.write_text(serialized + "\n", encoding="utf-8")
