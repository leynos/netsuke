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
release, every required result is present, every result the document declares
reports success, and no timestamp lies outside the bounded window. Treating a
declared failure in a result the gate does not yet require as invisible would
let a producer add a downstream canary and have its failure ignored. A manifest
that fails any check is refused with one bounded category rather than an
exception, so the caller records *why* without publishing a path, a revision,
or an API body.

Nothing here performs I/O: the classifiers take the instant the clock adapter
reported. That keeps the decision reviewable and testable without a filesystem
or a child process.
"""

import datetime as dt  # ruff: ignore[typing-only-standard-library-import] - examples.
import hashlib
import hmac
import re
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
#: The interim publication block (#598): only the exact text ``true`` lifts it,
#: and only after a live admitted and a live refused run are demonstrated.
PRODUCER_TRUST_VARIABLE = "NETSUKE_RELEASE_ADMISSION_PRODUCER_TRUSTED"
#: The SHA-256 of the manifest the producer job wrote, as that job published it
#: in its own output. Only this repository's workflow can name a job's output,
#: so a document whose bytes hash to it is the producer's; an unset, empty, or
#: malformed value attributes nothing, and the enforcing gate refuses.
EVIDENCE_DIGEST_VARIABLE = "NETSUKE_RELEASE_ADMISSION_EVIDENCE_SHA256"
#: The one spelling of a digest the producer publishes: lowercase hexadecimal.
_SHA256 = re.compile(r"[0-9a-f]{64}")

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

    A result the gate does not require is still read: a producer that declares
    a downstream canary has stated an outcome, and an unsuccessful statement is
    a refusal even though the name is outside :data:`policy.REQUIRED_RESULTS`.
    Only the *presence* rule is limited to the required set, so a producer may
    add canaries without a matching change here.

    Returns
    -------
    policy.Classification
        Success when the manifest names the candidate, declares every required
        result, and declares no unsuccessful one; otherwise the category of the
        first refusal.

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
    if any(name not in manifest.results for name in policy.REQUIRED_RESULTS):
        return policy.Classification(
            policy.Outcome.FAILURE, policy.ErrorCategory.MISSING_EVIDENCE
        )
    if any(
        observed.outcome != policy.Outcome.SUCCESS
        for observed in manifest.results.values()
    ):
        return policy.Classification(
            policy.Outcome.FAILURE, policy.ErrorCategory.UNKNOWN
        )
    return policy.Classification(policy.Outcome.SUCCESS, policy.ErrorCategory.NONE)


def is_attributed(document: bytes, published_digest: str) -> bool:
    """Return whether the document is the one the producer job published.

    The producer hashes the manifest it wrote and publishes the digest as a job
    output. A same-run upload under the artefact name cannot change that
    output, so only the producer's own bytes match it. The comparison is
    constant-time for tidiness rather than secrecy: the digest is public.

    Returns
    -------
    bool
        Whether the published digest is well formed and names these bytes.

    Examples
    --------
    >>> document = b"{}"
    >>> is_attributed(document, hashlib.sha256(document).hexdigest())
    True
    >>> is_attributed(document, "")
    False
    """
    if _SHA256.fullmatch(published_digest) is None:
        return False
    return hmac.compare_digest(hashlib.sha256(document).hexdigest(), published_digest)


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
    "EVIDENCE_DIGEST_VARIABLE",
    "EVIDENCE_FILE_DIAGNOSTIC",
    "EVIDENCE_FILE_VARIABLE",
    "MAX_AGE_VARIABLE",
    "MAX_EVIDENCE_AGE_DIGITS",
    "MAX_EVIDENCE_BYTES",
    "MAX_EVIDENCE_MAX_AGE_SECONDS",
    "MAX_EVIDENCE_SKEW_SECONDS",
    "PRODUCER_TRUST_VARIABLE",
    "TAG_DIAGNOSTIC",
    "TAG_VARIABLE",
    "Candidate",
    "Manifest",
    "ObservedResult",
    "Parsed",
    "classify_freshness",
    "is_attributed",
    "is_evidence_max_age",
    "parse_manifest",
    "verify",
)
