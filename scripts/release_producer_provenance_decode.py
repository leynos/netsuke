"""Decode a producer record's wire shape into a validated provenance claim.

A producer record is a JSON document written by one job and read by another, so
between the bytes and the value publication decides over there sit four
questions that are about the *document* rather than about the release: is this
a string-keyed object, does it carry every member, is its schema version one
this release understands, and is each member the type the format defines. They
are answered here.

:mod:`release_producer_provenance` owns the claim itself -- what a commit
looks like, which platforms can be packaged, and which asset names and digests
a record may claim. This module reads the document and passes its members on;
it never decides whether a value is *acceptable*, only whether it is *present,
of the right type, and written under a schema this reader knows*. That split is
what makes the refusals legible: a malformed document and a document making an
unacceptable claim are different diagnoses with different remedies.

Nothing here reads a file or parses JSON. The decoded object arrives from
:mod:`release_payload_tree`, which owns the tree scan, and the refusal
vocabulary lives in :mod:`release_producer_provenance_errors`.

Run via :func:`release_payload_tree.read_provenance_by_path` and
:func:`record_producer_provenance_record.record`; behavioural coverage lives
in ``scripts/tests/test_release_producer_provenance_decode.py``.
"""

import typing as typ

import release_producer_provenance
from release_producer_provenance import (
    ASSETS_MEMBER,
    COMMIT_MEMBER,
    PLATFORM_MEMBER,
    RUN_ATTEMPT_MEMBER,
    RUN_ID_MEMBER,
    SCHEMA_MEMBER,
    TARGET_MEMBER,
)
from release_producer_provenance_errors import (
    MalformedRecordError,
    MissingMemberError,
    NonMappingRecordError,
    SchemaNotIntegerError,
    UnknownSchemaError,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc

__all__ = ["SCHEMA_VERSION", "decode_provenance"]

#: The record schema this release understands. A producer writing a newer
#: schema is refused rather than partially read, so the version lives with the
#: reader that decides that, not with the claim.
SCHEMA_VERSION = 1

#: Every member a producer record must carry, in the order the writer emits
#: them. Restated from the policy module's vocabulary so a reader sees the
#: whole required set in one place.
_MEMBERS = (
    SCHEMA_MEMBER,
    COMMIT_MEMBER,
    RUN_ID_MEMBER,
    RUN_ATTEMPT_MEMBER,
    TARGET_MEMBER,
    PLATFORM_MEMBER,
    ASSETS_MEMBER,
)


def decode_provenance(record: object) -> release_producer_provenance.ProducerProvenance:
    """Read one decoded producer record and validate the claim it makes.

    Parameters
    ----------
    record
        The value ``json.loads`` returned for a producer record file.

    Returns
    -------
    release_producer_provenance.ProducerProvenance
        The validated claim.

    Notes
    -----
    The refusals reach the caller from the helpers below and from the policy
    module, so none appears in a ``Raises`` section -- Ruff's DOC501 reserves
    that section for exceptions a function raises itself -- but each is listed
    here so the contract is stated in one place:

    - :class:`NonMappingRecordError`: the document is not a string-keyed
      object.
    - :class:`MissingMemberError`: a required member is absent.
    - :class:`SchemaNotIntegerError`: the schema is not an integer.
    - :class:`UnknownSchemaError`: the schema is a version this release does
      not understand.
    - :class:`MalformedRecordError`: a text member is not a non-empty string,
      or the asset member is not an object.
    - :class:`release_producer_provenance_errors.ProvenanceError` and its
      subclasses: a member is present and textual but the claim it makes is
      unacceptable. Those come from the policy module, which is where the
      *claim* rather than the wire is judged.

    The asset map's *container* is checked here -- it must be an object --
    but its entries are passed on undecoded. Each key is data rather than a
    member, so judging a name and a digest is the policy module's business,
    and doing it here would put half of one validation in each file.

    Examples
    --------
    >>> record = {
    ...     "schema": 1, "commit": "a" * 40, "run_id": "7",
    ...     "run_attempt": "1", "target": "linux-x86_64",
    ...     "platform": "linux", "assets": {"netsuke.deb": "b" * 64},
    ... }
    >>> decode_provenance(record).target
    'linux-x86_64'
    """
    mapping = _string_keyed_mapping(record)
    _require_members(mapping)
    _require_understood_schema(mapping)
    return release_producer_provenance.validated_provenance(
        release_producer_provenance.RecordedMembers(
            commit=_required_text(mapping, COMMIT_MEMBER),
            run_id=_required_text(mapping, RUN_ID_MEMBER),
            run_attempt=_required_text(mapping, RUN_ATTEMPT_MEMBER),
            target=_required_text(mapping, TARGET_MEMBER),
            platform=_required_text(mapping, PLATFORM_MEMBER),
            assets=_required_object(mapping, ASSETS_MEMBER),
        )
    )


def _string_keyed_mapping(record: object) -> cabc.Mapping[str, object]:
    """Return the document as a string-keyed mapping, or refuse its type.

    Returns
    -------
    collections.abc.Mapping[str, object]
        The document, once its keys are all strings.

    Raises
    ------
    NonMappingRecordError
        If the document is not an object with string keys.

    Examples
    --------
    >>> _string_keyed_mapping({"schema": 1})["schema"]
    1
    """
    if not isinstance(record, dict) or not all(isinstance(key, str) for key in record):
        raise NonMappingRecordError(record)
    return record


def _require_members(record: cabc.Mapping[str, object]) -> None:
    """Refuse a document that does not carry every member publication needs.

    Raises
    ------
    MissingMemberError
        If any required member is absent.

    Examples
    --------
    >>> _require_members({"schema": 1, "commit": "a" * 40, "run_id": "7",
    ...                   "run_attempt": "1", "target": "t",
    ...                   "platform": "linux", "assets": {}})
    """
    missing = [member for member in _MEMBERS if member not in record]
    if missing:
        raise MissingMemberError(missing)


def _require_understood_schema(record: cabc.Mapping[str, object]) -> None:
    """Refuse a record whose schema version this release does not understand.

    Raises
    ------
    SchemaNotIntegerError
        If the schema is not an integer, or is a boolean.
    UnknownSchemaError
        If the schema is an integer but not the version this reader knows.

    Notes
    -----
    The version is judged here rather than carried into the claim because it
    describes the *document*: a claim has no schema, only values. Every caller
    that reads a record has to answer this question before any member means
    anything, so answering it once, at the decode step, is what keeps a newer
    producer's record from being partially read.

    ``True`` is an ``int`` in Python, so it is excluded explicitly: a record
    written by a template that substituted ``true`` for the version is not a
    record this reader may read as version one.

    Examples
    --------
    >>> _require_understood_schema({"schema": 1})
    >>> try:
    ...     _require_understood_schema({"schema": True})
    ... except SchemaNotIntegerError:
    ...     print("refused")
    refused
    >>> try:
    ...     _require_understood_schema({"schema": 2})
    ... except UnknownSchemaError:
    ...     print("unknown")
    unknown
    """
    value = record[SCHEMA_MEMBER]
    if not isinstance(value, int) or isinstance(value, bool):
        raise SchemaNotIntegerError(value)
    if value != SCHEMA_VERSION:
        raise UnknownSchemaError(value, SCHEMA_VERSION)


def _required_text(record: cabc.Mapping[str, object], member: str) -> str:
    """Return a non-empty string member, or refuse the document.

    Returns
    -------
    str
        The member's value.

    Raises
    ------
    MalformedRecordError
        If the member is not a non-empty string.

    Notes
    -----
    An empty string is refused rather than passed on. Every member this
    function reads is compared against the release later, and an empty value
    would make that comparison report a *stale candidate* when the real fault
    is a truncated document.

    Examples
    --------
    >>> _required_text({"target": "linux-x86_64"}, "target")
    'linux-x86_64'
    >>> try:
    ...     _required_text({"target": 7}, "target")
    ... except MalformedRecordError as refusal:
    ...     print("non-empty string" in str(refusal))
    True
    """
    value = record[member]
    if not isinstance(value, str) or not value:
        msg = f"the producer record {member} must be a non-empty string, got {value!r}"
        raise MalformedRecordError(msg)
    return value


def _required_object(
    record: cabc.Mapping[str, object], member: str
) -> cabc.Mapping[object, object]:
    """Return a member the format defines as an object, or refuse the document.

    Returns
    -------
    collections.abc.Mapping[object, object]
        The member's value, known to be a mapping but not yet judged. Its keys
        are typed as any object because a key is data: whether one names a
        publishable file is the policy's to decide.

    Raises
    ------
    MalformedRecordError
        If the member is not a mapping.

    Notes
    -----
    This is a *shape* check, not a claim check: the asset map's keys and values
    are data rather than members, so whether each names a publishable file or
    carries a hexadecimal digest is
    :func:`release_producer_assets.validated_assets`'s. Refusing the container
    here is what lets that function be typed over a mapping instead of
    testing its argument's type in a policy module.

    Examples
    --------
    >>> _required_object({"assets": {"a.deb": "b" * 64}}, "assets")
    {'a.deb': 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'}
    >>> try:
    ...     _required_object({"assets": ["a.deb"]}, "assets")
    ... except MalformedRecordError as refusal:
    ...     print("mapping" in str(refusal))
    True
    """
    value = record[member]
    if not isinstance(value, dict):
        msg = f"the producer record {member} must be a mapping, got {value!r}"
        raise MalformedRecordError(msg)
    return value
