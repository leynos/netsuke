"""Read and write the trusted digest record at release boundaries.

This adapter owns JSON serialization, the record's ``assets`` member, and path
errors for digest records. It delegates entry validation to
:class:`release_digest_record.DigestRecord`, so every consumer receives the
same validated record after decoding.

The declaration, upload, and inventory-verification CLIs use it in the release
workflow today. The finalization CLI uses it too and its suite passes, but no
workflow job invokes it yet.

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


class DuplicateKeyError(release_digest_record.DigestRecordError):
    """Report a JSON object that names one key more than once."""


def _object_without_duplicates(pairs: list[tuple[str, object]]) -> dict[str, object]:
    """Decode one JSON object, refusing a repeated key at any depth.

    Parameters
    ----------
    pairs
        The key and value pairs ``json`` decoded for one object.

    Returns
    -------
    dict[str, object]
        The object, when every key appeared once.

    Raises
    ------
    DuplicateKeyError
        If a key repeats, because ordinary decoding would silently keep the
        last value and hide the one that was discarded.

    Notes
    -----
    The hook runs for every object in the document, so a repeated ``assets``
    member and a repeated asset name are both refused. A record that repeats
    an entry cannot say which digest it means, which is the substitution the
    trusted record exists to catch.

    Examples
    --------
    >>> _object_without_duplicates([("a", 1)]) == {"a": 1}
    True
    >>> _object_without_duplicates([("a", 1), ("a", 2)])  # doctest: +ELLIPSIS
    Traceback (most recent call last):
        ...
    release_digest_record_files.DuplicateKeyError: the digest record repeats ...
    """
    decoded: dict[str, object] = {}
    for key, value in pairs:
        if key in decoded:
            msg = (
                f"the digest record repeats the JSON key {key!r}, so the value "
                "it names is ambiguous"
            )
            raise DuplicateKeyError(msg)
        decoded[key] = value
    return decoded


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
        decoded = json.loads(
            source.read_bytes(), object_pairs_hook=_object_without_duplicates
        )
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
    """Return the asset mapping from a decoded record.

    Returns
    -------
    dict[str, object]
        The unvalidated digest values from the record's ``assets`` member,
        keyed by asset name. Each shape check raises the error of the helper
        that performs it.

    Examples
    --------
    >>> _asset_mapping({"assets": {"a": "ab"}})
    {'a': 'ab'}
    """
    container = _require_mapping(record, "the digest record must be a mapping")
    declared = _require_mapping(
        container.get(ASSETS_MEMBER),
        f"the digest record must carry an '{ASSETS_MEMBER}' mapping",
    )
    return _string_keyed(declared)


def _require_mapping(value: object, description: str) -> dict[object, object]:
    """Return ``value`` as a mapping, or refuse the record shape.

    The description names which part of the record is malformed, so each
    refusal keeps the diagnosis it carried before the shape checks were
    split apart.

    Returns
    -------
    dict[object, object]
        The value, once it is known to be a mapping.

    Raises
    ------
    release_digest_record.DigestRecordError
        If the value is not a mapping.

    Examples
    --------
    >>> _require_mapping({"a": "ab"}, "the record must be a mapping")
    {'a': 'ab'}
    """
    if not isinstance(value, dict):
        msg = f"{description}, got {type(value).__name__}"
        raise release_digest_record.DigestRecordError(msg)
    return value


def _string_keyed(declared: dict[object, object]) -> dict[str, object]:
    """Return the asset mapping after refusing a non-string asset name.

    JSON object keys are always strings, so a non-string key can only arrive
    as an already decoded value. ``DigestRecord`` refuses such a key with the
    same message.

    Returns
    -------
    dict[str, object]
        The same entries, narrowed to string keys.

    Raises
    ------
    release_digest_record.DigestRecordError
        If any key is not a string.

    Examples
    --------
    >>> _string_keyed({"a": "ab"})
    {'a': 'ab'}
    """
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
