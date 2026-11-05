"""Read the provenance record one producer writes for the artefacts it built.

A declaration says *which names* a release publishes. It does not say which
commit produced them, and neither a package suffix nor a per-platform count
can establish that -- both describe a file, not its origin. Each producer
therefore records the candidate it built, the run it built under, and the
SHA-256 digest of every artefact it is handing on, and that record travels to
publication independently of the downloaded trees.

This module owns the record's shape and the reader that re-validates it. The
checks made *across* records, and against the release's own identity, belong to
:mod:`release_producer_provenance_checks`; the refusal vocabulary belongs to
:mod:`release_producer_provenance_errors`; and reading JSON, hashing files, and
scanning a downloaded tree belong to the adapters.

The record only binds bytes to a claim *by the producer*. It is evidence that
the job that ran under a given commit produced these digests; it is not a proof
that the commit was honest, and this module makes no such claim.

Run via the release publication adapters; behavioural coverage lives in
``scripts/tests/test_release_producer_provenance.py``.
"""

import dataclasses
import re
import typing as typ

import release_asset_names
import release_digest
import release_package_formats
from release_producer_provenance_errors import (
    EmptyAssetsError,
    InvalidDigestError,
    MalformedCommitError,
    MalformedRecordError,
    MalformedRunIdentityError,
    MissingMemberError,
    NonMappingRecordError,
    SchemaNotIntegerError,
    UnknownPlatformError,
    UnknownSchemaError,
    UnsafeAssetNameError,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc

__all__ = [
    "ASSETS_MEMBER",
    "COMMIT_MEMBER",
    "PLATFORM_MEMBER",
    "RUN_ATTEMPT_MEMBER",
    "RUN_ID_MEMBER",
    "SCHEMA_MEMBER",
    "SCHEMA_VERSION",
    "TARGET_MEMBER",
    "ProducerProvenance",
    "validate_provenance",
]

#: The record schema this module understands. A producer writing a newer schema
#: is refused rather than partially read.
SCHEMA_VERSION = 1

SCHEMA_MEMBER = "schema"
COMMIT_MEMBER = "commit"
RUN_ID_MEMBER = "run_id"
RUN_ATTEMPT_MEMBER = "run_attempt"
TARGET_MEMBER = "target"
PLATFORM_MEMBER = "platform"
ASSETS_MEMBER = "assets"

#: Every member a producer record must carry.
_MEMBERS = (
    SCHEMA_MEMBER,
    COMMIT_MEMBER,
    RUN_ID_MEMBER,
    RUN_ATTEMPT_MEMBER,
    TARGET_MEMBER,
    PLATFORM_MEMBER,
    ASSETS_MEMBER,
)

#: A candidate is a full lowercase hexadecimal commit. The same rule
#: :mod:`release_candidate` applies, restated here so a producer record is
#: judged without importing the resolver that consumes it.
_COMMIT = re.compile(r"\A[0-9a-f]{40}\Z")

#: Run identities are decimal counters. They are compared as text only, so the
#: rule exists to refuse an empty or malformed claim, not to do arithmetic.
_RUN_IDENTITY = re.compile(r"\A[1-9][0-9]*\Z")

#: One SHA-256 digest, lowercase hexadecimal.
_DIGEST = re.compile(rf"\A[0-9a-f]{{{release_digest.DIGEST_HEX_LENGTH}}}\Z")


@dataclasses.dataclass(frozen=True, slots=True)
class ProducerProvenance:
    """One producer's claim about the artefacts it built.

    Attributes
    ----------
    schema
        Record schema version, equal to :data:`SCHEMA_VERSION`.
    commit
        The full lowercase commit the producer checked out and built.
    run_id
        The workflow run that produced the artefacts.
    run_attempt
        The attempt within that run. A retried job may legitimately hand on
        fresh bytes, so this is recorded but not compared.
    target
        The configured staging target the producer served.
    platform
        The platform whose package formats the target stages.
    assets
        Published asset name mapped to its lowercase SHA-256 digest.
    """

    schema: int
    commit: str
    run_id: str
    run_attempt: str
    target: str
    platform: str
    assets: dict[str, str]


def validate_provenance(record: object) -> ProducerProvenance:
    """Return the provenance a decoded record describes, re-validated in full.

    Parameters
    ----------
    record
        Decoded producer record, normally parsed by the JSON adapter.

    Returns
    -------
    ProducerProvenance
        The validated claim.

    Notes
    -----
    Commit *shape* is checked here; whether it is *this release's* candidate is
    checked by :func:`release_producer_provenance_checks.require_candidate`,
    because a well-formed record for an older commit is a different diagnosis
    from a malformed one.

    The shape checks are delegated to private helpers, each of which raises
    the diagnosis it owns: ``MalformedRecordError``, ``MissingMemberError``,
    ``UnknownPlatformError``, ``UnsafeAssetNameError``, ``InvalidDigestError``,
    and ``EmptyAssetsError`` all surface from this call.

    Examples
    --------
    >>> record = {
    ...     "schema": 1, "commit": "a" * 40, "run_id": "7",
    ...     "run_attempt": "1", "target": "linux-x86_64",
    ...     "platform": "linux",
    ...     "assets": {"netsuke.deb": "b" * 64},
    ... }
    >>> validate_provenance(record).target
    'linux-x86_64'
    """
    mapping = _string_keyed_mapping(record)
    _require_members(mapping)
    target = _required_text(mapping, TARGET_MEMBER)
    return ProducerProvenance(
        schema=_schema_version(mapping),
        commit=_commit(mapping),
        run_id=_required_run_identity(mapping, RUN_ID_MEMBER),
        run_attempt=_required_run_identity(mapping, RUN_ATTEMPT_MEMBER),
        target=target,
        platform=_platform(mapping),
        assets=_validated_assets(mapping[ASSETS_MEMBER], target=target),
    )


def _string_keyed_mapping(record: object) -> cabc.Mapping[str, object]:
    """Return the record as a string-keyed mapping, or refuse its type.

    Returns
    -------
    collections.abc.Mapping[str, object]
        The record, once its keys are all strings.

    Raises
    ------
    NonMappingRecordError
        If the record is not a mapping with string keys.

    Examples
    --------
    >>> _string_keyed_mapping({"schema": 1})["schema"]
    1
    """
    if not isinstance(record, dict) or not all(isinstance(key, str) for key in record):
        raise NonMappingRecordError(record)
    return record


def _require_members(record: cabc.Mapping[str, object]) -> None:
    """Refuse a record that does not carry every member publication needs.

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


def _schema_version(record: cabc.Mapping[str, object]) -> int:
    """Return the record's schema version, or refuse an unknown one.

    Returns
    -------
    int
        The validated schema version.

    Raises
    ------
    SchemaNotIntegerError
        If the schema is not an integer.
    UnknownSchemaError
        If the schema is not the version this reader understands.

    Examples
    --------
    >>> _schema_version({"schema": 1})
    1
    """
    schema = record[SCHEMA_MEMBER]
    if not isinstance(schema, int) or isinstance(schema, bool):
        raise SchemaNotIntegerError(schema)
    if schema != SCHEMA_VERSION:
        raise UnknownSchemaError(schema, SCHEMA_VERSION)
    return schema


def _commit(record: cabc.Mapping[str, object]) -> str:
    """Return the record's commit, or refuse one of the wrong shape.

    Returns
    -------
    str
        The full lowercase commit the record claims.

    Raises
    ------
    MalformedCommitError
        If the commit is not a full lowercase hexadecimal commit.

    Examples
    --------
    >>> _commit({"commit": "a" * 40})[:8]
    'aaaaaaaa'
    """
    commit = _required_text(record, COMMIT_MEMBER)
    if not _COMMIT.match(commit):
        raise MalformedCommitError(commit)
    return commit


def _required_run_identity(record: cabc.Mapping[str, object], member: str) -> str:
    """Return one run-identity member, or refuse a non-numeric value.

    Returns
    -------
    str
        The member's positive-integer text.

    Raises
    ------
    MalformedRunIdentityError
        If the member is not positive-integer text.

    Examples
    --------
    >>> _required_run_identity({"run_attempt": "2"}, "run_attempt")
    '2'
    """
    value = _required_text(record, member)
    if not _RUN_IDENTITY.match(value):
        raise MalformedRunIdentityError(member, value)
    return value


def _platform(record: cabc.Mapping[str, object]) -> str:
    """Return the record's platform, or refuse one nothing can package.

    Returns
    -------
    str
        The platform whose package formats the record names.

    Raises
    ------
    UnknownPlatformError
        If the platform stages no package format.

    Examples
    --------
    >>> _platform({"platform": "linux"})
    'linux'
    """
    platform = _required_text(record, PLATFORM_MEMBER)
    if platform not in release_package_formats.PLATFORM_FORMATS:
        raise UnknownPlatformError(platform)
    return platform


def _required_text(record: cabc.Mapping[str, object], member: str) -> str:
    """Return a non-empty string member, or refuse the record.

    Returns
    -------
    str
        The member's value.

    Raises
    ------
    MalformedRecordError
        If the member is not a non-empty string.

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


def _validated_assets(declared: object, *, target: str) -> dict[str, str]:
    """Return a validated name-to-digest mapping for one producer.

    Returns
    -------
    dict[str, str]
        Published asset name mapped to its lowercase digest.

    Raises
    ------
    MalformedRecordError
        If ``declared`` is not a mapping.
    EmptyAssetsError
        If it holds no entry.
    UnsafeAssetNameError
        If a name is not one bare file name.
    InvalidDigestError
        If a digest is malformed.

    Examples
    --------
    >>> _validated_assets({"a.deb": "b" * 64}, target="linux-x86_64")
    {'a.deb': 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'}
    """
    if not isinstance(declared, dict):
        msg = (
            f"the producer record assets must be a mapping, got "
            f"{type(declared).__name__}"
        )
        raise MalformedRecordError(msg)
    if not declared:
        raise EmptyAssetsError(target)
    assets: dict[str, str] = {}
    for name, digest in declared.items():
        if not isinstance(name, str) or not release_asset_names.is_bare_file_name(name):
            raise UnsafeAssetNameError(target, str(name))
        if not isinstance(digest, str) or not _DIGEST.match(digest):
            raise InvalidDigestError(target, name)
        assets[name] = digest
    return assets
