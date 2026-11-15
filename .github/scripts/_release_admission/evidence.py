"""Decide whether an evidence manifest admits the candidate under release.

RFC 0005 splits release admission in two. Producers record what they observed;
admission verifies what they recorded rather than trusting a claim about it.
This module is the verifying half: it takes the manifest :mod:`manifest` parsed
and the candidate the workflow is about to release, and answers with one
bounded classification. It also owns the bounded maximum age the freshness
check runs under, and the variables that configure it.

The manifest is the whole input. It names the repository, commit, tag and
workflow run it was produced for, when it was issued, and one result per
canary. Admission accepts it only when those values name the candidate under
release, every required result is present and successful, and no timestamp
lies outside the bounded window. A manifest that fails any check is refused
with one bounded category rather than an exception, so the caller records
*why* without publishing a path, a revision, or an API body.

Nothing here performs I/O: the classifiers take the instant the clock adapter
reported. That keeps the decision reviewable and testable without a filesystem
or a child process.
"""

import datetime as dt  # ruff: ignore[typing-only-standard-library-import] - examples.
import typing as typ

from . import policy
from .manifest import Candidate, Manifest, ObservedResult, Parsed, parse_manifest

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: Environment variables naming the candidate the manifest must describe and
#: the document itself. They are owned here, beside the schema they select,
#: rather than in :mod:`configuration`, which only reads them.
EVIDENCE_FILE_VARIABLE = "NETSUKE_RELEASE_ADMISSION_EVIDENCE_FILE"
EVIDENCE_FILE_DIAGNOSTIC = (
    "NETSUKE_RELEASE_ADMISSION_EVIDENCE_FILE must name the evidence manifest"
)
TAG_VARIABLE = "NETSUKE_RELEASE_ADMISSION_TAG"
TAG_DIAGNOSTIC = "NETSUKE_RELEASE_ADMISSION_TAG must name the release tag"
MAX_AGE_VARIABLE = "NETSUKE_RELEASE_ADMISSION_MAX_EVIDENCE_AGE_SECONDS"

#: Largest manifest the gate will read. The document is a handful of short
#: fields, so the bound is generous; what it stops is a misconfigured path
#: naming a build artefact, or a producer writing without end, from being
#: pulled into memory before the gate has decided anything.
MAX_EVIDENCE_BYTES = 64 * 1024
#: How old an evidence timestamp may be before the gate refuses it. A candidate
#: admitted on day-old observations is not the candidate the workflow resolved,
#: so the default is one day and the ceiling is one week.
DEFAULT_EVIDENCE_MAX_AGE_SECONDS = 86_400
MAX_EVIDENCE_MAX_AGE_SECONDS = 604_800
#: Longest run of digits a maximum age may contain before it is rejected.
MAX_EVIDENCE_AGE_DIGITS = 6
#: How far ahead of the gate's own clock a producer's timestamp may sit. Runner
#: clocks are not synchronised, so a few minutes of lead is expected; a larger
#: one means the document was written for a later moment than the release it
#: claims to describe.
MAX_EVIDENCE_SKEW_SECONDS = 300


def classify_freshness(
    manifest: Manifest, *, now: float, max_age_seconds: int
) -> policy.Classification:
    """Classify whether every recorded moment lies inside the bounded window.

    Both ends are checked. A timestamp older than ``max_age_seconds`` says the
    observations predate the release by more than the policy allows; a
    timestamp further ahead than :attr:`policy.MAX_EVIDENCE_SKEW_SECONDS` says
    the document was written for a moment that has not arrived. Runner clocks
    are not synchronised, so a small lead is expected and accepted.

    Returns
    -------
    policy.Classification
        Success when every moment is within the window, otherwise a stale
        evidence refusal.

    Examples
    --------
    >>> issued = dt.datetime(2026, 1, 1, tzinfo=dt.UTC)
    >>> manifest = Manifest(
    ...     repository="leynos/netsuke",
    ...     commit="a" * 40,
    ...     tag="v0.10.0",
    ...     workflow_run_id="1001",
    ...     issued_at=issued,
    ...     results={"history_scan": ObservedResult("success", issued)},
    ... )
    >>> classify_freshness(
    ...     manifest, now=issued.timestamp() + 60, max_age_seconds=3600
    ... ).outcome
    <Outcome.SUCCESS: 'success'>
    """
    for moment in _moments(manifest):
        age = now - moment.timestamp()
        if age > max_age_seconds or -age > MAX_EVIDENCE_SKEW_SECONDS:
            return policy.Classification(
                policy.Outcome.FAILURE, policy.ErrorCategory.STALE_EVIDENCE
            )
    return policy.Classification(policy.Outcome.SUCCESS, policy.ErrorCategory.NONE)


def verify(manifest: Manifest, candidate: Candidate) -> policy.Classification:
    """Classify whether a manifest admits the candidate under release.

    Identity is checked before results, so a manifest describing another
    release is reported as a mismatch even when its own results are complete:
    the operator's next step is to find the right document, not to re-run the
    canaries. The tag is compared only when the candidate names one, which is
    the rehearsal case where no tag is being published.

    Returns
    -------
    policy.Classification
        Success when the manifest names the candidate and reports every
        required result as successful; otherwise the category of the first
        refusal.

    Examples
    --------
    >>> issued = dt.datetime(2026, 1, 1, tzinfo=dt.UTC)
    >>> results = {
    ...     name: ObservedResult("success", issued)
    ...     for name in policy.REQUIRED_RESULTS
    ... }
    >>> manifest = Manifest(
    ...     repository="leynos/netsuke",
    ...     commit="a" * 40,
    ...     tag="v0.10.0",
    ...     workflow_run_id="1001",
    ...     issued_at=issued,
    ...     results=results,
    ... )
    >>> candidate = Candidate(
    ...     repository="leynos/netsuke",
    ...     commit="a" * 40,
    ...     tag="v0.10.0",
    ...     run_id="1001",
    ... )
    >>> verify(manifest, candidate).outcome
    <Outcome.SUCCESS: 'success'>
    """
    if _describes_another_release(manifest, candidate):
        return policy.Classification(
            policy.Outcome.FAILURE, policy.ErrorCategory.CANDIDATE_MISMATCH
        )
    for name in policy.REQUIRED_RESULTS:
        observed = manifest.results.get(name)
        if observed is None:
            return policy.Classification(
                policy.Outcome.FAILURE, policy.ErrorCategory.MISSING_EVIDENCE
            )
        if observed.outcome != policy.Outcome.SUCCESS:
            return policy.Classification(
                policy.Outcome.FAILURE, policy.ErrorCategory.UNKNOWN
            )
    return policy.Classification(policy.Outcome.SUCCESS, policy.ErrorCategory.NONE)


def is_evidence_max_age(value: str) -> bool:
    """Return whether a value is an acceptable maximum evidence age.

    The shape mirrors :func:`policy.is_operation_timeout`: ASCII digits, no
    leading zero, bounded length, and a value of at most one week. The bound
    exists so a mistyped maximum cannot turn the freshness check off.

    Returns
    -------
    bool
        Whether the value is an acceptable bounded maximum age.

    Examples
    --------
    >>> is_evidence_max_age("86400"), is_evidence_max_age("0")
    (True, False)
    """
    return (
        value.isascii()
        and value.isdigit()
        and value[0] != "0"
        and len(value) <= MAX_EVIDENCE_AGE_DIGITS
        and int(value) <= MAX_EVIDENCE_MAX_AGE_SECONDS
    )


def _describes_another_release(manifest: Manifest, candidate: Candidate) -> bool:
    """Return whether any bound field of the manifest names something else."""
    recorded = (
        (manifest.repository, candidate.repository),
        (manifest.commit, candidate.commit),
        (manifest.workflow_run_id, candidate.run_id),
    )
    if any(value != expected for value, expected in recorded):
        return True
    return bool(candidate.tag) and manifest.tag != candidate.tag


def _moments(manifest: Manifest) -> cabc.Iterator[dt.datetime]:
    """Yield the manifest's issue time and every observation time."""
    yield manifest.issued_at
    for observed in manifest.results.values():
        yield observed.observed_at


__all__ = (
    "DEFAULT_EVIDENCE_MAX_AGE_SECONDS",
    "EVIDENCE_FILE_DIAGNOSTIC",
    "EVIDENCE_FILE_VARIABLE",
    "MAX_AGE_VARIABLE",
    "MAX_EVIDENCE_AGE_DIGITS",
    "MAX_EVIDENCE_BYTES",
    "MAX_EVIDENCE_MAX_AGE_SECONDS",
    "MAX_EVIDENCE_SKEW_SECONDS",
    "TAG_DIAGNOSTIC",
    "TAG_VARIABLE",
    "Candidate",
    "Manifest",
    "ObservedResult",
    "Parsed",
    "classify_freshness",
    "is_evidence_max_age",
    "parse_manifest",
    "verify",
)
