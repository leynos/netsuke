"""Represent trusted digest records and compare their coverage.

The release record maps published asset names to SHA-256 digests. This module
owns the validated :class:`DigestRecord` and the comparisons made against it.
JSON parsing, the record's ``assets`` member, serialization, and file access
belong to :mod:`release_digest_record_files`.

The record is written before upload and remains independent of downloaded
bytes. Comparing downloads to it is what makes replacing an archive or package
visible even when a replacement sidecar agrees with the replacement bytes.

Run via the release publication adapters; behavioural coverage lives in
``scripts/tests/test_release_digest_record.py``.
"""

import dataclasses
import re
import typing as typ

import release_digest

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: One hexadecimal SHA-256 digest, in either case.
_DIGEST_TEXT = re.compile(rf"\A[0-9a-fA-F]{{{release_digest.DIGEST_HEX_LENGTH}}}\Z")


class DigestRecordError(ValueError):
    """Report a digest record that cannot support a verification decision."""


class RecordedDigestMismatchError(
    DigestRecordError, release_digest.DigestMismatchError
):
    """Report downloaded bytes that differ from the trusted record."""


@dataclasses.dataclass(frozen=True, slots=True)
class DigestRecord:
    """A non-empty, validated set of published asset names and digests."""

    #: Asset names paired with their lowercase SHA-256 digest, sorted by name.
    entries: tuple[tuple[str, str], ...]

    def __post_init__(self) -> None:
        """Refuse a record that is empty or carries a malformed entry.

        The comparison functions accept any :class:`DigestRecord`, so the
        guarantee that a record is usable has to hold for every construction
        path rather than only for the factory. Validation here is what makes
        "validated record" a property of the type.

        Raises
        ------
        DigestRecordError
            If the record declares no asset, names an asset with a non-string
            key, lists an asset name more than once, or carries a digest that
            is not a lowercase hexadecimal value.

        Examples
        --------
        >>> DigestRecord((("a", "ab" * 32),)).names()
        {'a'}
        >>> DigestRecord(())  # doctest: +ELLIPSIS
        Traceback (most recent call last):
            ...
        release_digest_record.DigestRecordError: the digest record declares no assets...
        """
        if not self.entries:
            msg = "the digest record declares no assets, so there is nothing to verify"
            raise DigestRecordError(msg)
        for name, digest in self.entries:
            if _validated_entry(name, digest) != (name, digest):
                msg = f"the digest record must carry lowercase digests, got {digest!r}"
                raise DigestRecordError(msg)
        _require_unique_names(self.entries)

    @classmethod
    def from_mapping(cls, declared: cabc.Mapping[str, object]) -> DigestRecord:
        """Validate decoded entries and build the trusted record.

        Parameters
        ----------
        declared
            Asset names mapped to digests, before validation.

        Returns
        -------
        DigestRecord
            The entries with lowercase digests, sorted by asset name.

        Notes
        -----
        This factory normalizes case and sorts; the value invariant itself is
        enforced by :meth:`__post_init__`, whose ``DigestRecordError`` reaches
        the caller unchanged.

        Examples
        --------
        >>> DigestRecord.from_mapping({"a": "AB" * 32}).as_mapping()["a"] == "ab" * 32
        True
        >>> DigestRecord.from_mapping({})  # doctest: +ELLIPSIS
        Traceback (most recent call last):
            ...
        release_digest_record.DigestRecordError: the digest record declares no assets...
        """
        return cls(tuple(sorted(_validated_entries(declared))))

    def as_mapping(self) -> dict[str, str]:
        """Return the record as a name-to-digest mapping.

        Returns
        -------
        dict[str, str]
            Each asset name mapped to its lowercase digest.

        Examples
        --------
        >>> DigestRecord.from_mapping({"a": "ab" * 32}).as_mapping()["a"] == "ab" * 32
        True
        """
        return dict(self.entries)

    def names(self) -> set[str]:
        """Return the asset names the record covers.

        Returns
        -------
        set[str]
            The asset names, without their digests.

        Examples
        --------
        >>> DigestRecord.from_mapping({"a": "ab" * 32}).names()
        {'a'}
        """
        return {name for name, _ in self.entries}


def _validated_entries(
    declared: cabc.Mapping[str, object],
) -> cabc.Iterable[tuple[str, str]]:
    """Yield each asset name and lowercase digest after validating its types.

    Yields
    ------
    tuple[str, str]
        One asset name and its lowercase digest.

    Notes
    -----
    :func:`_validated_entry` raises ``DigestRecordError`` for a malformed
    entry. Its check runs as each entry is yielded, so a malformed key is
    refused before ``sorted`` compares it against a string key. Sorting first
    would raise ``TypeError`` for mixed key types instead of the record's own
    error.

    Examples
    --------
    >>> list(_validated_entries({"a": "AB" * 32}))[0][1] == "ab" * 32
    True
    """
    for name, digest in declared.items():
        yield _validated_entry(name, digest)


def _require_unique_names(entries: cabc.Iterable[tuple[str, str]]) -> None:
    """Refuse a record that names one asset more than once.

    A repeated name is reachable only by constructing the record directly,
    because a mapping cannot repeat a key. Every accessor collapses the
    repetition, so only the later digest would be compared and the earlier
    digest would never be checked against anything.

    Raises
    ------
    DigestRecordError
        If two entries share an asset name.

    Examples
    --------
    >>> _require_unique_names([("a", "ab" * 32), ("b", "ab" * 32)])
    >>> _require_unique_names(  # doctest: +ELLIPSIS
    ...     [("a", "ab" * 32), ("a", "ab" * 32)]
    ... )
    Traceback (most recent call last):
        ...
    release_digest_record.DigestRecordError: the digest record must list each...
    """
    names = [name for name, _ in entries]
    if len(set(names)) != len(names):
        msg = "the digest record must list each asset name once"
        raise DigestRecordError(msg)


def _validated_entry(name: object, digest: object) -> tuple[str, str]:
    """Return one asset name and its lowercase digest, or refuse the entry.

    Returns
    -------
    tuple[str, str]
        The asset name and its lowercase digest.

    Raises
    ------
    DigestRecordError
        If either value is not a string, or the digest is not a valid
        hexadecimal value.

    Examples
    --------
    >>> _validated_entry("a", "AB" * 32)[1] == "ab" * 32
    True
    """
    if not isinstance(name, str) or not isinstance(digest, str):
        msg = "the digest record must map asset names to digests as strings"
        raise DigestRecordError(msg)
    return name, _validated_digest(name, digest)


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
    record: DigestRecord, *, expected: cabc.Iterable[str]
) -> None:
    """Require the record to cover exactly the assets the release declares.

    Raises
    ------
    DigestRecordError
        If the declaration is empty, or the two sets differ in either
        direction.

    Examples
    --------
    >>> require_record_matches_declaration(
    ...     DigestRecord.from_mapping({"a": "ab" * 32}), expected=["a"]
    ... )
    >>> require_record_matches_declaration(  # doctest: +ELLIPSIS
    ...     DigestRecord.from_mapping({"a": "ab" * 32}), expected=["a", "b"]
    ... )
    Traceback (most recent call last):
        ...
    release_digest_record.DigestRecordError: ...
    """
    declared = set(expected)
    if not declared:
        msg = "the declaration names no asset, so there is nothing to verify"
        raise DigestRecordError(msg)
    _require_complete_coverage(declared, record.names())


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
    computed: cabc.Mapping[str, str], *, trusted: DigestRecord
) -> None:
    """Require every downloaded asset digest to match the trusted record.

    Raises
    ------
    RecordedDigestMismatchError
        If any trusted asset is missing from the computed mapping or has a
        different digest.

    Examples
    --------
    >>> require_downloads_match_record(
    ...     {"a": "ab" * 32}, trusted=DigestRecord.from_mapping({"a": "ab" * 32})
    ... )
    >>> require_downloads_match_record(  # doctest: +ELLIPSIS
    ...     {"a": "cd" * 32}, trusted=DigestRecord.from_mapping({"a": "ab" * 32})
    ... )
    Traceback (most recent call last):
        ...
    release_digest_record.RecordedDigestMismatchError: uploaded assets do not match...
    """
    wrong = _mismatched_download_names(computed, trusted.as_mapping())
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
