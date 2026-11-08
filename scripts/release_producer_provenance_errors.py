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

The two renderers below exist because the same sentence shape recurred. One
names a summary and the single value it fixes; the other names the producer's
target and the asset at fault. Centralizing them is not cosmetic: every
diagnosis here is pinned by a doctest or a test, so a change to how a value is
quoted must reach all of them at once rather than one class at a time.

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
    "TargetClaimMismatchError",
    "UnknownPlatformError",
    "UnknownSchemaError",
    "UnsafeAssetNameError",
]


def _quoted_tail(summary: str, value: object) -> str:
    """Join a diagnosis summary to the quoted value it names.

    Parameters
    ----------
    summary
        The sentence stem, ending where the value belongs.
    value
        The offending value, rendered so its text is visible literally.

    Returns
    -------
    str
        The summary followed by the value's ``repr``.

    Examples
    --------
    >>> _quoted_tail("the schema must be an integer, got", 3)
    'the schema must be an integer, got 3'
    """
    return f"{summary} {value!r}"


def _target_asset_diagnosis(summary: str, target: str, name: str) -> str:
    """Name the producer's target and the asset at fault in one sentence.

    Parameters
    ----------
    summary
        What the producer did with the asset, such as
        ``"claims unsafe asset name"``.
    target
        The configured target whose producer made the claim.
    name
        The asset name the claim concerned.

    Returns
    -------
    str
        The composed diagnosis.

    Examples
    --------
    >>> _target_asset_diagnosis("claims unsafe asset name", "linux", "../x")
    "producer for target 'linux' claims unsafe asset name '../x'"
    """
    return f"producer for target {target!r} {summary} {name!r}"


class ProvenanceError(ValueError):
    """Report a producer record that cannot support a publication decision."""


class _OneValueError(ProvenanceError):
    """Report a refusal whose message ends in one quoted value.

    A subclass declares the sentence stem as ``_summary``; this class appends
    the value. Five diagnoses share the shape ``<stem> <value>``, and writing
    that forwarding once keeps the stem beside the class that owns it without
    repeating the same ``__init__`` in each.
    """

    _summary: str

    def __init__(self, value: object) -> None:
        """Build the diagnosis from the declared stem and ``value``."""
        super().__init__(_quoted_tail(self._summary, value))


class _TargetAssetError(ProvenanceError):
    """Report a refusal naming the producer's target and the asset at fault.

    A subclass declares the verb phrase as ``_summary``; this class composes
    ``producer for target <target> <summary> <asset>``.
    """

    _summary: str

    def __init__(self, target: str, name: str) -> None:
        """Build the diagnosis naming ``target`` and ``name``."""
        super().__init__(_target_asset_diagnosis(self._summary, target, name))


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


class SchemaNotIntegerError(_OneValueError, MalformedRecordError):
    """Report a record whose schema member is not an integer."""

    _summary = "the producer record schema must be an integer, got"


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


class MalformedCommitError(_OneValueError, MalformedRecordError):
    """Report a record whose commit is not a full lowercase commit."""

    _summary = "the producer record commit must be a full lowercase commit, got"


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


class UnsafeAssetNameError(_TargetAssetError):
    """Report a claimed asset name that is not one bare file name."""

    _summary = "claims unsafe asset name"


class UnknownPlatformError(_OneValueError):
    """Report a record whose platform stages no package format at all."""

    _summary = "producer record declares unpackageable platform"


class InvalidDigestError(_TargetAssetError):
    """Report a recorded digest that is not a lowercase SHA-256 value."""

    _summary = "records a malformed digest for"


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


class TargetClaimMismatchError(ProvenanceError):
    """Report a producer whose claims are not the ones its target stages.

    A record names the target it served, so its archive, that archive's
    sidecar, and the package formats its platform stages are all knowable
    before any name is compared with another record's. Checking the union of
    claims alone would let two same-platform records exchange their claims
    and still agree with the release.

    Examples
    --------
    >>> error = TargetClaimMismatchError(["target 'a' claims archives []"])
    >>> "producer claims are not their own target's" in str(error)
    True
    """

    def __init__(self, offenders: list[str]) -> None:
        """Build the diagnosis naming every disagreeing target at once.

        Parameters
        ----------
        offenders
            Human-readable descriptions, in target order.
        """
        super().__init__(
            "producer claims are not their own target's: " + "; ".join(offenders)
        )


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
