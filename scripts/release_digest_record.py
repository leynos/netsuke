"""Validate trusted digest-record values and compare their coverage.

The release record maps published asset names to SHA-256 digests. This module
owns that in-memory shape and the comparisons made against it. JSON parsing,
serialization, and file access belong to :mod:`release_digest_record_files`.

The record is written before upload and remains independent of downloaded
bytes. Comparing downloads to it is what makes replacing an archive or package
visible even when a replacement sidecar agrees with the replacement bytes.

Run via the release publication adapters; behavioural coverage lives in
``scripts/tests/test_release_digest_record.py``.
"""

import re
import typing as typ

import release_digest

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: One hexadecimal SHA-256 digest, in either case.
_DIGEST_TEXT = re.compile(rf"\A[0-9a-fA-F]{{{release_digest.DIGEST_HEX_LENGTH}}}\Z")

#: The member holding the name-to-digest mapping.
ASSETS_MEMBER = "assets"


class DigestRecordError(ValueError):
    """Report a digest record that cannot support a verification decision."""


class RecordedDigestMismatchError(
    DigestRecordError, release_digest.DigestMismatchError
):
    """Report downloaded bytes that differ from the trusted record."""


def validate_digest_record(record: object) -> dict[str, str]:
    """Return a validated name-to-lowercase-digest mapping.

    Parameters
    ----------
    record
        Decoded record value, normally parsed by the JSON adapter.

    Returns
    -------
    dict[str, str]
        Asset name mapped to its lowercase SHA-256 digest.

    Notes
    -----
    The validation helpers' :class:`DigestRecordError` reaches the caller
    unchanged, preserving the precise reason the decoded record was refused.

    Examples
    --------
    >>> validate_digest_record({"assets": {"a": "AB" * 32}})["a"] == "ab" * 32
    True
    >>> validate_digest_record({"assets": {}})  # doctest: +ELLIPSIS
    Traceback (most recent call last):
        ...
    release_digest_record.DigestRecordError: the digest record declares no assets...
    """
    declared = _asset_mapping(record)
    return dict(_validated_entries(declared))


def _asset_mapping(record: object) -> dict[object, object]:
    """Return the non-empty asset mapping from a decoded record.

    Returns
    -------
    dict[object, object]
        The unvalidated entries from the record's ``assets`` member.

    Raises
    ------
    DigestRecordError
        If the record is not a non-empty mapping of asset entries.

    Examples
    --------
    >>> _asset_mapping({"assets": {"a": "ab"}})
    {'a': 'ab'}
    """
    if not isinstance(record, dict):
        msg = f"the digest record must be a mapping, got {type(record).__name__}"
        raise DigestRecordError(msg)
    declared = record.get(ASSETS_MEMBER)
    if not isinstance(declared, dict):
        msg = (
            f"the digest record must carry an '{ASSETS_MEMBER}' mapping, got "
            f"{type(declared).__name__}"
        )
        raise DigestRecordError(msg)
    if not declared:
        msg = "the digest record declares no assets, so there is nothing to verify"
        raise DigestRecordError(msg)
    return declared


def _validated_entries(
    declared: dict[object, object],
) -> cabc.Iterable[tuple[str, str]]:
    """Yield each asset name and lowercase digest after validating its types.

    Yields
    ------
    tuple[str, str]
        One asset name and its lowercase digest.

    Raises
    ------
    DigestRecordError
        If an entry is not a string name and a valid hexadecimal digest.

    Examples
    --------
    >>> list(_validated_entries({"a": "AB" * 32}))[0][1] == "ab" * 32
    True
    """
    for name, digest in declared.items():
        if not isinstance(name, str) or not isinstance(digest, str):
            msg = "the digest record must map asset names to digests as strings"
            raise DigestRecordError(msg)
        yield name, _validated_digest(name, digest)


def _validated_digest(name: str, digest: str) -> str:
    """Return one lowercase SHA-256 digest or refuse the malformed value.

    Returns
    -------
    str
        The lowercase digest associated with ``name``.

    Raises
    ------
    DigestRecordError
        If ``digest`` is not a 64-character hexadecimal SHA-256 value.

    Examples
    --------
    >>> _validated_digest("a", "AB" * 32) == "ab" * 32
    True
    """
    if not _DIGEST_TEXT.fullmatch(digest):
        msg = f"the digest record holds a non-digest for {name!r}: {digest!r}"
        raise DigestRecordError(msg)
    return digest.lower()


def require_record_matches_declaration(
    digests: cabc.Mapping[str, str], *, expected: cabc.Iterable[str]
) -> None:
    """Require the record to cover exactly the assets the release declares.

    Raises
    ------
    DigestRecordError
        If the declaration is empty, or the two sets differ in either
        direction.

    Examples
    --------
    >>> require_record_matches_declaration({"a": "ab"}, expected=["a"])
    >>> require_record_matches_declaration(  # doctest: +ELLIPSIS
    ...     {"a": "ab"}, expected=["a", "b"]
    ... )
    Traceback (most recent call last):
        ...
    release_digest_record.DigestRecordError: ...
    """
    declared = set(expected)
    if not declared:
        msg = "the declaration names no asset, so there is nothing to verify"
        raise DigestRecordError(msg)
    _require_complete_coverage(declared, set(digests))


def _require_complete_coverage(declared: set[str], recorded: set[str]) -> None:
    """Require the record to name exactly the assets the release declares."""
    missing = sorted(declared - recorded)
    extra = sorted(recorded - declared)
    if not missing and not extra:
        return
    msg = (
        "the digest record and the declaration disagree: "
        f"record omits {missing!r}, record adds {extra!r}"
    )
    raise DigestRecordError(msg)


def require_downloads_match_record(
    computed: cabc.Mapping[str, str], *, trusted: cabc.Mapping[str, str]
) -> None:
    """Require every downloaded asset digest to match the trusted record.

    Raises
    ------
    RecordedDigestMismatchError
        If any trusted asset is missing from the computed mapping or has a
        different digest.

    Examples
    --------
    >>> require_downloads_match_record({"a": "ab"}, trusted={"a": "ab"})
    >>> require_downloads_match_record(  # doctest: +ELLIPSIS
    ...     {"a": "cd"}, trusted={"a": "ab"}
    ... )
    Traceback (most recent call last):
        ...
    release_digest_record.RecordedDigestMismatchError: uploaded assets do not match...
    """
    wrong = _mismatched_download_names(computed, trusted)
    if not wrong:
        return
    msg = f"uploaded assets do not match the staged candidate: {wrong!r}"
    raise RecordedDigestMismatchError(msg)


def _mismatched_download_names(
    computed: cabc.Mapping[str, str], trusted: cabc.Mapping[str, str]
) -> list[str]:
    """Return every missing or differing asset in sorted order.

    Returns
    -------
    list[str]
        Trusted asset names absent from or different in ``computed``.

    Examples
    --------
    >>> _mismatched_download_names({"a": "cd"}, {"a": "ab", "b": "ef"})
    ['a', 'b']
    """
    absent = {name for name in trusted if name not in computed}
    differing = {
        name
        for name, recorded in trusted.items()
        if name in computed
        and not release_digest.constant_time_equal(computed[name], recorded)
    }
    return sorted(absent | differing)
