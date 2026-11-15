"""Parse the candidate-bound release-admission evidence manifest.

RFC 0005 splits release admission in two. Producers record what they observed;
admission verifies what they recorded rather than trusting a claim about it.
This module owns the document itself: its schema version, the field readers
that decode it, and the single refusal every unreadable document collapses to.
:mod:`evidence` owns the decisions taken over it -- whether its timestamps are
fresh and whether it admits the candidate.

The two are separate modules because they change for different reasons. A
producer that changes a field's meaning bumps ``schema_version``, and only this
module has to learn the new shape; the admission rules stay as they were. The
split also keeps each module inside the repository's module-length ceiling.

Nothing here performs I/O. :func:`parse_manifest` takes bytes another module
read through the filesystem port, so the schema is reviewable and testable
without a filesystem or a child process.

The schema is versioned rather than merely shaped. A document carrying a
version this module does not know is refused as malformed instead of being
interpreted under the wrong rules. Unknown *extra* fields and unknown *result
names* are accepted, because #598 adds downstream canaries and a verifier that
rejected every name it had not been taught would make each one a breaking
change.
"""

import dataclasses
import datetime as dt
import json
import re
import typing as typ

from . import policy

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: The only manifest schema this verifier understands.
SCHEMA_VERSION = 1

#: A commit is compared, and reported, as the full lowercase hexadecimal form
#: the workflow's metadata job already validated.
_COMMIT = re.compile(r"[0-9a-f]{40}")


@dataclasses.dataclass(frozen=True, slots=True)
class ObservedResult:
    """Carry one canary's recorded outcome and the moment it was observed."""

    outcome: str
    observed_at: dt.datetime


@dataclasses.dataclass(frozen=True, slots=True)
class Manifest:
    """Carry a parsed, not yet trusted, evidence manifest."""

    repository: str
    commit: str
    tag: str
    workflow_run_id: str
    issued_at: dt.datetime
    results: cabc.Mapping[str, ObservedResult]


class Candidate(typ.NamedTuple):
    """Name the release a manifest must describe to admit this run.

    The four values travel together through every identity comparison, so they
    are one value rather than four arguments: a caller cannot pass the tag
    where the run identifier belongs, and a rehearsal states an empty tag once.
    """

    repository: str
    commit: str
    tag: str
    run_id: str


class Parsed(typ.NamedTuple):
    """Hold a parsed manifest, or the category explaining why there is none."""

    manifest: Manifest | None
    error_category: policy.ErrorCategory


def parse_manifest(raw: bytes) -> Parsed:
    """Parse manifest bytes into a manifest, or into a refusal category.

    Every way the document can be unusable -- not UTF-8, not JSON, not an
    object, a missing or mistyped field, an unknown schema version -- collapses
    to :attr:`policy.ErrorCategory.MALFORMED_EVIDENCE`. They are one operator
    problem: a producer wrote something this verifier cannot read, and the run
    needs a corrected document rather than a finer diagnosis of the old one.

    Returns
    -------
    Parsed
        The manifest with no error category, or a refusal category with no
        manifest.

    Examples
    --------
    >>> parse_manifest(b"{}").error_category
    <ErrorCategory.MALFORMED_EVIDENCE: 'malformed_evidence'>
    >>> parse_manifest(b"not json").manifest is None
    True
    """
    try:
        document = json.loads(raw)
    except ValueError, RecursionError:
        # `json.loads` reports both a UTF-8 failure and a syntax error as a
        # `ValueError`, and admits nothing else here. A decoder that refuses a
        # deeply nested document raises `RecursionError` instead, which is not
        # a `ValueError`; it is the same operator problem -- a document this
        # verifier cannot read -- so it takes the same refusal rather than
        # escaping as an unclassified failure of the gate.
        return _refused()
    manifest = _manifest(document)
    if manifest is None:
        return _refused()
    return Parsed(manifest, policy.ErrorCategory.NONE)


def _refused() -> Parsed:
    """Build the refusal returned for every unreadable document."""
    return Parsed(None, policy.ErrorCategory.MALFORMED_EVIDENCE)


def _manifest(document: object) -> Manifest | None:
    """Build a manifest from a decoded document, or return ``None``.

    The document is read as two halves -- the release it names and the
    observations it records -- because a refusal is about one half or the
    other, and each half is a value that only exists when all of its fields are
    usable. Assembling them is what leaves this function two conditions wide.

    Returns
    -------
    Manifest | None
        The parsed manifest, or ``None`` when the document is not one.
    """
    if not isinstance(document, dict):
        return None
    if not _is_current_schema(document.get("schema_version")):
        return None
    identity = _identity(document)
    observation = _observation(document)
    if identity is None or observation is None:
        return None
    issued_at, results = observation
    return Manifest(
        repository=identity.repository,
        commit=identity.commit,
        tag=identity.tag,
        workflow_run_id=identity.run_id,
        issued_at=issued_at,
        results=results,
    )


def _identity(document: cabc.Mapping[str, object]) -> Candidate | None:
    """Return the release fields the document names, or ``None``.

    Returns
    -------
    Candidate | None
        The repository, commit, tag and run identifier it names, or ``None``
        when any of them is missing or mistyped.
    """
    commit = _commit(document.get("commit"))
    repository = _text(document.get("repository"))
    tag = _text(document.get("tag"))
    run_id = _identifier(document.get("workflow_run_id"))
    if commit is None or repository is None:
        return None
    if tag is None or run_id is None:
        return None
    return Candidate(repository, commit, tag, run_id)


def _observation(
    document: cabc.Mapping[str, object],
) -> tuple[dt.datetime, dict[str, ObservedResult]] | None:
    """Return the issue time and the results, or ``None`` when one is unusable.

    Returns
    -------
    tuple | None
        The moment the document was issued with every result it records, or
        ``None`` when either half cannot be read.
    """
    issued_at = _timestamp(document.get("issued_at"))
    results = _results(document.get("results"))
    if issued_at is None or results is None:
        return None
    return issued_at, results


def _commit(value: object) -> str | None:
    """Return a full lowercase object ID, or ``None`` for anything else.

    Returns
    -------
    str | None
        The identifier in the canonical form the metadata job validated, or
        ``None`` when the value is not one.
    """
    text = _text(value)
    if text is None:
        return None
    return text if _COMMIT.fullmatch(text) else None


def _is_current_schema(value: object) -> bool:
    """Return whether a value is this version, and not a bool that equals it."""
    return isinstance(value, int) and not isinstance(value, bool) and value == 1


def _text(value: object) -> str | None:
    """Return a non-empty string, or ``None`` for anything else."""
    if isinstance(value, str) and value:
        return value
    return None


def _identifier(value: object) -> str | None:
    """Return a workflow run identifier in its canonical string form.

    Producers may write the identifier as a JSON number, which is how GitHub
    itself reports it, or as a string, which survives a large integer's
    printing rules unambiguously. Both name the same run once normalised.

    Returns
    -------
    str | None
        The identifier as text, or ``None`` when the value names no run.
    """
    match value:
        case bool():
            return None
        case int() as number if number > 0:
            return str(number)
        case _:
            return _text(value)


def _timestamp(value: object) -> dt.datetime | None:
    """Return an RFC 3339 UTC instant, or ``None`` for anything else.

    Only UTC is accepted, written either as ``Z`` or as ``+00:00``. A local
    offset would make the freshness comparison depend on which end of the
    conversion an arithmetic bug landed, so the schema refuses it rather than
    normalising silently.

    Returns
    -------
    dt.datetime | None
        The instant in UTC, or ``None`` when the value is not a UTC timestamp.
    """
    if not isinstance(value, str):
        return None
    try:
        moment = dt.datetime.fromisoformat(value)
    except ValueError:
        return None
    if moment.tzinfo is None or moment.utcoffset() != dt.timedelta(0):
        return None
    return moment.astimezone(dt.UTC)


def _results(value: object) -> dict[str, ObservedResult] | None:
    """Return the manifest's results, or ``None`` when one is malformed.

    The outcome must be a member of :class:`policy.Outcome`; a producer that
    invents a word has written a document this verifier cannot act on, which
    is a different failure from reporting ``failure``. Result *names* are not
    checked, so a downstream canary can add one without changing this module.

    Returns
    -------
    dict[str, ObservedResult] | None
        Every result the document records, or ``None`` when one of them is not
        a usable entry.
    """
    if not isinstance(value, dict):
        return None
    results: dict[str, ObservedResult] = {}
    for name, record in value.items():
        observed = _observed_result(name, record)
        if observed is None:
            return None
        results[name] = observed
    return results


def _observed_result(name: object, record: object) -> ObservedResult | None:
    """Return one recorded result, or ``None`` when the entry is unusable.

    The entry is validated as a whole rather than field by field at the call
    site, so :func:`_results` stays a loop over entries and the refusals stay
    together where a reader looks for them.

    Returns
    -------
    ObservedResult | None
        The result the entry records, or ``None`` when its name, outcome, or
        observation time is not something this verifier can act on.
    """
    if not isinstance(name, str) or not name:
        return None
    if not isinstance(record, dict):
        return None
    outcome = record.get("outcome")
    if not isinstance(outcome, str) or not policy.Outcome.contains(outcome):
        return None
    observed_at = _timestamp(record.get("observed_at"))
    if observed_at is None:
        return None
    return ObservedResult(outcome=outcome, observed_at=observed_at)


__all__ = (
    "SCHEMA_VERSION",
    "Candidate",
    "Manifest",
    "ObservedResult",
    "Parsed",
    "parse_manifest",
)
