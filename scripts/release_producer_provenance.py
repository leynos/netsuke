"""Read the provenance record one producer writes for the artefacts it built.

A declaration says *which names* a release publishes, not which commit
produced them, and neither a package suffix nor a per-platform count can
establish that: both describe a file, not its origin. Each producer therefore
records the candidate it built, the run it built under, and the SHA-256 digest
of every artefact it hands on, and that record reaches publication
independently of the downloaded trees.

This module owns the record's shape and the reader that re-validates it.
Cross-record checks belong to :mod:`release_producer_provenance_checks`, the
refusal vocabulary to :mod:`release_producer_provenance_errors`, and JSON,
hashing, and tree scanning to the adapters.

The record binds bytes to a claim *by the producer*: evidence the job running
under a given commit produced these digests, not proof the commit was honest.
Coverage: ``scripts/tests/test_release_producer_provenance.py``.
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

#: A full lowercase hexadecimal commit: the rule :mod:`release_candidate`
#: applies, restated so a record is judged without the resolver that reads it.
_COMMIT = re.compile(r"\A[0-9a-f]{40}\Z")

#: Run identities are decimal counters, compared as text only: the rule
#: refuses a malformed claim rather than doing arithmetic.
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
        Published name mapped to its lowercase SHA-256 digest.
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

    Returns
    -------
    ProducerProvenance
        The validated claim.

    Notes
    -----
    Commit *shape* is checked here; whether it is *this release's* candidate
    is :func:`release_producer_provenance_checks.require_candidate`'s, because
    a well-formed record for an older commit is a different diagnosis from a
    malformed one.

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

    Notes
    -----
    ``UnsafeAssetNameError`` and ``InvalidDigestError`` surface from the two
    helpers below.

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
    return {
        _validated_name(name, target=target): _validated_digest(
            name, digest, target=target
        )
        for name, digest in declared.items()
    }


def _validated_name(name: object, *, target: str) -> str:
    """Return one claimed asset name, refusing anything but a bare file name.

    Returns
    -------
    str
        The name, once it is a string a release may publish.

    Raises
    ------
    UnsafeAssetNameError
        If the name is not a string, or names anything but one bare file.

    Notes
    -----
    The name is judged first because the digest's refusal quotes it, and a
    value that is not a string cannot be quoted as one.
    """
    if not isinstance(name, str) or not release_asset_names.is_bare_file_name(name):
        raise UnsafeAssetNameError(target, str(name))
    return name


def _validated_digest(name: str, digest: object, *, target: str) -> str:
    """Return the digest claimed for ``name``, refusing a malformed one.

    Returns
    -------
    str
        The lowercase SHA-256 digest the producer recorded.

    Raises
    ------
    InvalidDigestError
        If the digest is not a string of the expected hexadecimal length.
    """
    if not isinstance(digest, str) or not _DIGEST.match(digest):
        raise InvalidDigestError(target, name)
    return digest
