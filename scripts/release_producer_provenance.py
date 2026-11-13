"""Read the provenance record one producer writes for the artefacts it built.

A declaration says *which names* a release publishes, not which commit
produced them, and neither a package suffix nor a per-platform count can
establish that: both describe a file, not its origin. Each producer therefore
records the candidate it built, the run it built under, and the SHA-256 digest
of every artefact it hands on.

The record travels with that producer's own artefacts: the artifact upload
carries the directory the step wrote into, and publication downloads all of
them back into one tree. What makes the record independent is *where* it is
kept, not how it arrives -- it is never derived from the downloaded bytes it
describes, and a tree that reaches publication without it is refused rather
than trusted.

This module owns the claim a record makes: what a commit and a run identity
look like, which platforms can be packaged, and that the record names at least
one asset. Reading the *document* -- that it is an object, that it carries its
members, that each member is the type the format defines, and that its schema
version is one this release understands -- is
:mod:`release_producer_provenance_decode`'s, so a value arriving here is
already the right type and every refusal below is about the claim rather than
about the wire. The asset map it carries is
:mod:`release_producer_assets`'s, cross-record checks belong to
:mod:`release_producer_provenance_checks`, the refusal vocabulary to
:mod:`release_producer_provenance_errors`, and JSON, hashing, and tree
scanning to the adapters.

The record binds bytes to a claim *by the producer*: evidence the job running
under a given commit produced these digests, not proof the commit was honest.
Coverage: ``scripts/tests/test_release_producer_provenance.py``.
"""

import dataclasses
import re

import release_package_formats
import release_producer_assets
from release_producer_provenance_errors import (
    MalformedCommitError,
    MalformedRunIdentityError,
    UnknownPlatformError,
)

__all__ = [
    "ASSETS_MEMBER",
    "COMMIT_MEMBER",
    "PLATFORM_MEMBER",
    "RUN_ATTEMPT_MEMBER",
    "RUN_ID_MEMBER",
    "SCHEMA_MEMBER",
    "TARGET_MEMBER",
    "ProducerProvenance",
    "RecordedMembers",
    "validated_provenance",
]

#: The record's member names. The writer composes a document from them and the
#: decoder reads it back, so they live here rather than in either: one
#: spelling, shared by the two halves that would otherwise drift apart.
SCHEMA_MEMBER = "schema"
COMMIT_MEMBER = "commit"
RUN_ID_MEMBER = "run_id"
RUN_ATTEMPT_MEMBER = "run_attempt"
TARGET_MEMBER = "target"
PLATFORM_MEMBER = "platform"
ASSETS_MEMBER = "assets"

#: A full lowercase hexadecimal commit: the rule :mod:`release_candidate`
#: applies, restated so a record is judged without the resolver that reads it.
_COMMIT = re.compile(r"\A[0-9a-f]{40}\Z")

#: Run identities are decimal counters, compared as text only: the rule
#: refuses a malformed claim rather than doing arithmetic.
_RUN_IDENTITY = re.compile(r"\A[1-9][0-9]*\Z")


@dataclasses.dataclass(frozen=True, slots=True)
class ProducerProvenance:
    """One producer's claim about the artefacts it built.

    The record's schema version is not a member: it describes the *document*
    the claim arrived in, and the decoder has already refused a version this
    reader does not understand by the time a claim exists. Carrying it here
    would invite a caller to compare it against something.

    Attributes
    ----------
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

    commit: str
    run_id: str
    run_attempt: str
    target: str
    platform: str
    assets: dict[str, str]


@dataclasses.dataclass(frozen=True, slots=True)
class RecordedMembers:
    """Hold a producer record's decoded members before they are judged.

    The members are one document's fields, so they travel as one value rather
    than as seven parameters: a caller that transposed two of them would pass
    type checking and produce a claim naming another run's artefacts.

    Attributes
    ----------
    commit
        The commit the producer claims to have built.
    run_id
        The workflow run that produced the artefacts.
    run_attempt
        The attempt within that run.
    target
        The configured staging target the producer served.
    platform
        The platform whose package formats the target stages.
    assets
        The decoded asset map, whose shape the policy decides.
    """

    commit: str
    run_id: str
    run_attempt: str
    target: str
    platform: str
    assets: object


def validated_provenance(members: RecordedMembers) -> ProducerProvenance:
    """Return the claim these decoded members describe, or refuse it.

    Parameters
    ----------
    members
        One record's decoded members, as
        :func:`release_producer_provenance_decode.decode_provenance` hands
        them over.

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

    The members arrive already typed by
    :func:`release_producer_provenance_decode.decode_provenance`, so every
    refusal here is about the *claim*: a commit that is not hexadecimal, a
    run identity that is not a counter, a platform with no package format.
    A caller holding a raw document decodes it first rather than reaching
    this function.

    Those refusals come from the private helpers below, so none appears in a
    ``Raises`` section -- Ruff's DOC501 reserves that section for exceptions a
    function raises itself -- but each is listed here so the contract is
    stated in one place:

    - :class:`MalformedCommitError`: the commit is not a full lowercase
      hexadecimal commit.
    - :class:`MalformedRunIdentityError`: a run identity is not
      positive-integer text.
    - :class:`UnknownPlatformError`: the platform stages no package format.
    - :class:`release_producer_assets.AssetClaimError`: the asset map is not
      a usable claim, as judged by
      :func:`release_producer_assets.validated_assets`.

    Examples
    --------
    >>> validated_provenance(RecordedMembers(
    ...     commit="a" * 40, run_id="7", run_attempt="1",
    ...     target="linux-x86_64", platform="linux",
    ...     assets={"netsuke.deb": "b" * 64},
    ... )).target
    'linux-x86_64'
    """
    return ProducerProvenance(
        commit=_commit(members.commit),
        run_id=_required_run_identity(members.run_id, RUN_ID_MEMBER),
        run_attempt=_required_run_identity(members.run_attempt, RUN_ATTEMPT_MEMBER),
        target=members.target,
        platform=_platform(members.platform),
        assets=release_producer_assets.validated_assets(
            members.assets, target=members.target
        ),
    )


def _commit(commit: str) -> str:
    """Return the claimed commit, or refuse one of the wrong shape.

    Returns
    -------
    str
        The full lowercase commit the record claims.

    Raises
    ------
    MalformedCommitError
        If the commit is not a full lowercase hexadecimal commit.
    """
    if not _COMMIT.match(commit):
        raise MalformedCommitError(commit)
    return commit


def _required_run_identity(value: str, member: str) -> str:
    """Return one run identity, or refuse a non-numeric claim.

    Returns
    -------
    str
        The member's positive-integer text.

    Raises
    ------
    MalformedRunIdentityError
        If the member is not positive-integer text.
    """
    if not _RUN_IDENTITY.match(value):
        raise MalformedRunIdentityError(member, value)
    return value


def _platform(platform: str) -> str:
    """Return the claimed platform, or refuse one nothing can package.

    Returns
    -------
    str
        The platform whose package formats the record names.

    Raises
    ------
    UnknownPlatformError
        If the platform stages no package format.
    """
    if platform not in release_package_formats.PLATFORM_FORMATS:
        raise UnknownPlatformError(platform)
    return platform
