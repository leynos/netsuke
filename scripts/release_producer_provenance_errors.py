"""Name every way a producer record can fail publication.

A producer record is read by three parties: the build lane that writes it, the
selection adapter that copies the bytes it describes, and the uploader that
reconciles those bytes against the draft release. All three need the same
diagnosis vocabulary, and none of them should have to import the record reader
merely to name a refusal.

Messages live in the exception classes rather than at the raise sites, so a
caller composes a typed refusal and never an ad-hoc string. This module decides
nothing: it holds no schema version, no digest rule, and no record reader, and
it imports nothing. That is what lets both sides of the record's own split
depend on it without a cycle.

Kept apart from :mod:`release_producer_provenance` so each file stays under the
repository's 400-line cap.

Run via the release publication adapters; behavioural coverage lives in
``scripts/tests/test_release_producer_provenance.py``.
"""

__all__ = [
    "DuplicateAssetError",
    "DuplicateTargetError",
    "EmptyAssetsError",
    "ForeignRunError",
    "InvalidDigestError",
    "MalformedCommitError",
    "MalformedRecordError",
    "MalformedRunIdentityError",
    "MissingMemberError",
    "MissingTargetError",
    "NonMappingRecordError",
    "ProvenanceError",
    "SchemaNotIntegerError",
    "StaleCandidateError",
    "UnknownPlatformError",
    "UnknownSchemaError",
    "UnsafeAssetNameError",
]


class ProvenanceError(ValueError):
    """Report a producer record that cannot support a publication decision."""


class MalformedRecordError(ProvenanceError):
    """Report a record that is not a mapping of the expected shape."""


class NonMappingRecordError(MalformedRecordError):
    """Report a value that is not a string-keyed mapping.

    Examples
    --------
    >>> str(NonMappingRecordError(None))
    'the producer record must be a string-keyed mapping, got NoneType'
    """

    def __init__(self, record: object) -> None:
        """Build the diagnosis naming the value's type.

        Parameters
        ----------
        record
            The value that arrived where a record was expected.
        """
        super().__init__(
            "the producer record must be a string-keyed mapping, got "
            f"{type(record).__name__}"
        )


class SchemaNotIntegerError(MalformedRecordError):
    """Report a record whose schema member is not an integer."""

    def __init__(self, schema: object) -> None:
        """Build the diagnosis naming the schema value.

        Parameters
        ----------
        schema
            The schema value the record carried.
        """
        super().__init__(
            f"the producer record schema must be an integer, got {schema!r}"
        )


class UnknownSchemaError(MalformedRecordError):
    """Report a record written to a schema this reader does not know."""

    def __init__(self, schema: object, expected: int) -> None:
        """Build the diagnosis naming both schema versions.

        Parameters
        ----------
        schema
            The schema value the record carried.
        expected
            The schema version this reader understands.
        """
        super().__init__(f"the producer record schema is {schema!r}, not {expected}")


class MalformedCommitError(MalformedRecordError):
    """Report a record whose commit is not a full lowercase commit."""

    def __init__(self, commit: str) -> None:
        """Build the diagnosis naming the refused commit.

        Parameters
        ----------
        commit
            The commit text the record carried.
        """
        super().__init__(
            "the producer record commit must be a full lowercase commit, got "
            f"{commit!r}"
        )


class MalformedRunIdentityError(MalformedRecordError):
    """Report a record whose run identity is not positive-integer text."""

    def __init__(self, member: str, value: str) -> None:
        """Build the diagnosis naming the member and its value.

        Parameters
        ----------
        member
            Which run-identity member was refused.
        value
            The value the record carried.
        """
        super().__init__(
            f"the producer record {member} must be a positive integer, got {value!r}"
        )


class MissingMemberError(ProvenanceError):
    """Report a record that does not carry every member publication needs.

    Examples
    --------
    >>> str(MissingMemberError(["run_id"]))
    "producer record is missing ['run_id']"
    """

    def __init__(self, missing: list[str]) -> None:
        """Build the diagnosis naming the absent members.

        Parameters
        ----------
        missing
            Member names the record did not carry.
        """
        super().__init__(f"producer record is missing {missing!r}")


class StaleCandidateError(ProvenanceError):
    """Report a record whose commit is not the release's candidate.

    Examples
    --------
    >>> message = str(StaleCandidateError("a" * 40, "b" * 40))
    >>> message.startswith("producer built commit 'aaaa")
    True
    >>> message.endswith("not candidate 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'")
    True
    """

    def __init__(self, built: str, candidate: str) -> None:
        """Build the diagnosis naming both commits.

        Parameters
        ----------
        built
            The commit the producer recorded.
        candidate
            The commit the release declared.
        """
        super().__init__(
            f"producer built commit {built!r}, not candidate {candidate!r}"
        )


class ForeignRunError(ProvenanceError):
    """Report a record carried over from another workflow run or attempt."""

    def __init__(self, recorded: str, *, expected: str, member: str) -> None:
        """Build the diagnosis naming the member and both values.

        Parameters
        ----------
        recorded
            The value the record carried.
        expected
            The value the current run declares.
        member
            Which run-identity member disagreed.
        """
        super().__init__(
            f"producer record was written under {member} {recorded!r}, "
            f"not the current run's {expected!r}"
        )


class UnsafeAssetNameError(ProvenanceError):
    """Report a claimed asset name that is not one bare file name."""

    def __init__(self, target: str, name: str) -> None:
        """Build the diagnosis naming the producer and the refused name.

        Parameters
        ----------
        target
            The configured target whose producer made the claim.
        name
            The name the producer claimed.
        """
        super().__init__(
            f"producer for target {target!r} claims unsafe asset name {name!r}"
        )


class UnknownPlatformError(ProvenanceError):
    """Report a record whose platform stages no package format at all."""

    def __init__(self, platform: object) -> None:
        """Build the diagnosis naming the platform.

        Parameters
        ----------
        platform
            The value the record carried.
        """
        super().__init__(
            f"producer record declares unpackageable platform {platform!r}"
        )


class InvalidDigestError(ProvenanceError):
    """Report a recorded digest that is not a lowercase SHA-256 value."""

    def __init__(self, target: str, name: str) -> None:
        """Build the diagnosis naming the producer and the digest's asset.

        Parameters
        ----------
        target
            The configured target whose producer made the claim.
        name
            The asset name whose digest was malformed.
        """
        super().__init__(
            f"producer for target {target!r} records a malformed digest for {name!r}"
        )


class EmptyAssetsError(ProvenanceError):
    """Report a producer that claimed no artefacts at all."""

    def __init__(self, target: str) -> None:
        """Build the diagnosis naming the producer's target.

        Parameters
        ----------
        target
            The configured target whose producer recorded no assets.
        """
        super().__init__(f"producer record for target {target!r} claims no assets")


class MissingTargetError(ProvenanceError):
    """Report a configured target no producer recorded."""

    def __init__(self, missing: list[str]) -> None:
        """Build the diagnosis naming the uncovered targets.

        Parameters
        ----------
        missing
            Configured target keys with no producer record.
        """
        super().__init__(f"no producer record covers targets {missing!r}")


class DuplicateTargetError(ProvenanceError):
    """Report two producer records claiming one configured target."""

    def __init__(self, duplicated: list[str]) -> None:
        """Build the diagnosis naming the doubly covered targets.

        Parameters
        ----------
        duplicated
            Configured target keys more than one record claims.
        """
        super().__init__(f"more than one producer record covers targets {duplicated!r}")


class DuplicateAssetError(ProvenanceError):
    """Report two producers claiming the same published asset name."""

    def __init__(self, names: list[str]) -> None:
        """Build the diagnosis naming the doubly claimed assets.

        Parameters
        ----------
        names
            Published names claimed more than once.
        """
        super().__init__(
            "more than one producer claims published assets " + repr(names)
        )
