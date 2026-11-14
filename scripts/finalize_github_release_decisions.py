"""Decide what a finalization run may do, from values alone.

Every decision the finalizer makes passes through this module. A recorded plan,
a release snapshot, and the trusted digest record go in; a
:class:`PublishOutcome` comes out. Nothing here reads a file, runs a command,
composes operator prose, or names a process exit status. That separation is
what lets the decision be exercised with plain values: a test can ask what the
run decides about a public release with a changed asset without a mocked
``gh``, and the answer is the same outcome the command line turns into prose.

Prose and statuses live at the command edge. ``finalize_github_release`` maps
each verdict to the workflow's exit vocabulary and renders the diagnosis, so
changing how a refusal reads cannot change what the step does. The two facts an
operator most needs travel as data rather than inside a sentence: which assets
stopped the run, and whether any read established that the release is still a
draft. :attr:`PublishOutcome.is_draft` is ``None`` when nothing established it,
which is not the same claim as "not a draft".

What a read established and what an edit replied travel as the values and port
protocols in ``finalize_github_release_ports``, so this module remains a
statement of policy: what the run decides, never how it asks.

The vocabulary below is shared by every decision in this package. Judging a
release snapshot against the record — including the two gates the pre-edit read
serves — lives in ``finalize_github_release_inventory``, so this module holds
only what a verdict is and how a plan or a failed call becomes one.

Run via ``scripts/finalize_github_release.py``; behavioural coverage lives in
``scripts/tests/test_finalize_github_release_publish_decisions.py`` and
``scripts/tests/test_finalize_github_release_publish_ports.py``.
"""

import dataclasses
import enum
import typing as typ

import finalize_github_release_plan as release_plan

if typ.TYPE_CHECKING:
    # The state appears only in an annotation: this module judges a snapshot
    # another module read, and never constructs or inspects one at runtime.
    import finalize_github_release_state as release_state


class PublishVerdict(enum.Enum):
    """Name what a finalization run may do next.

    ``VERIFIED`` means the step may proceed; every other member names why it
    stops or what it established once the edit was attempted.
    """

    #: Every declared asset holds the trusted digest, so the run may proceed.
    VERIFIED = "verified"

    #: A read taken after the edit found the release public and matching.
    FINALIZED = "finalized"

    #: A retry found the release already public with the candidate's flag.
    ALREADY_FINALIZED = "already-finalized"

    #: The plan still expects an upload, which this stage never performs.
    UPLOAD_REQUIRED = "upload-required"

    #: The draft carries an attachment the trusted record never declared.
    UNDECLARED_ASSET = "undeclared-asset"

    #: An asset disagrees, or its digest could not be established.
    REFUSED = "refused"

    #: The plan document itself could not be decoded.
    INPUT_UNREADABLE = "input-unreadable"

    #: A GitHub call failed or answered with something unusable.
    REMOTE_FAILED = "remote-failed"

    #: The edit was accepted but the release is still a draft.
    STILL_DRAFT = "still-draft"

    #: The release is already public, so there is nothing to un-draft.
    NOT_A_DRAFT = "not-a-draft"

    #: The public release's prerelease state contradicts the candidate.
    PRERELEASE_MISMATCH = "prerelease-mismatch"

    #: The public release no longer holds the trusted inventory.
    PUBLISHED_MISMATCH = "published-mismatch"


@dataclasses.dataclass(frozen=True, slots=True)
class PublishOutcome:
    """Carry one decision, the assets it rests on, and what is known.

    Attributes
    ----------
    verdict
        What the run may do next, or what it established after the edit.
    source
        What the assessment was made from, so a refusal can name a stale plan
        rather than a release that changed under the run. It is a label, not a
        diagnosis.
    uploads
        Declared assets the assessment still expects to upload.
    conflicts
        Asset names whose bytes disagree with the trusted record.
    unverifiable
        Asset names whose digest the release did not report usably.
    undeclared
        Asset names attached to the release that the trusted record never
        declared. They are named here because the record cannot describe them
        at all, so no digest comparison can: the run reports the attachment
        rather than vouching for it.
    detail
        The source's own explanation: why a plan could not be decoded, or the
        clause naming the command that failed.
    is_draft
        Whether a read established that the release is still a draft. ``None``
        when nothing established it, which claims less than ``False``.
    after_edit
        Whether an un-drafting call had been attempted when the failure was
        observed.

    Examples
    --------
    >>> outcome = PublishOutcome(PublishVerdict.REFUSED, conflicts=("a.deb",))
    >>> outcome.verdict.value, outcome.conflicts
    ('refused', ('a.deb',))
    """

    verdict: PublishVerdict
    source: str = ""
    uploads: tuple[str, ...] = ()
    conflicts: tuple[str, ...] = ()
    unverifiable: tuple[str, ...] = ()
    undeclared: tuple[str, ...] = ()
    detail: str = ""
    is_draft: bool | None = None
    after_edit: bool = False


def assess(
    assessment: release_plan.PlanAssessment,
    *,
    source: str = "the reconciliation plan",
    draft_confirmed: bool = False,
) -> PublishOutcome:
    """Turn a plan assessment into the outcome that follows from it.

    Parameters
    ----------
    assessment
        The verdict, and the assets each verdict rests on.
    source
        What the assessment was made from, carried into any refusal.
    draft_confirmed
        Whether a read taken for this run established that the release is
        still a draft. A refusal made from the plan alone passes ``False``,
        because it has read nothing and a retry may begin after an earlier
        attempt published the release.

    Returns
    -------
    PublishOutcome
        :attr:`PublishVerdict.VERIFIED` when the run may proceed, otherwise
        the refusal with the assets that produced it.

    Examples
    --------
    >>> from finalize_github_release_plan import PlanAssessment, PlanVerdict
    >>> assess(PlanAssessment(PlanVerdict.VERIFIED)).verdict.value
    'verified'
    >>> outcome = assess(
    ...     PlanAssessment(PlanVerdict.UPLOAD_REQUIRED, uploads=("a.deb",))
    ... )
    >>> outcome.verdict.value, outcome.uploads
    ('upload-required', ('a.deb',))
    """
    if assessment.verdict is release_plan.PlanVerdict.VERIFIED:
        return PublishOutcome(PublishVerdict.VERIFIED, source=source)
    if assessment.verdict is release_plan.PlanVerdict.UPLOAD_REQUIRED:
        return PublishOutcome(
            PublishVerdict.UPLOAD_REQUIRED,
            source=source,
            uploads=assessment.uploads,
            is_draft=True if draft_confirmed else None,
        )
    if assessment.verdict is release_plan.PlanVerdict.UNREADABLE:
        return PublishOutcome(
            PublishVerdict.INPUT_UNREADABLE,
            source=source,
            detail=assessment.detail,
        )
    return PublishOutcome(
        PublishVerdict.REFUSED,
        source=source,
        conflicts=assessment.conflicts,
        unverifiable=assessment.unverifiable,
        detail=assessment.detail,
        is_draft=True if draft_confirmed else None,
    )


def draft_gate(state: release_state.ReleaseState) -> PublishOutcome:
    """Refuse to edit a release that has already been published.

    A public release has nothing to un-draft, and one with the wrong
    prerelease flag cannot be corrected without making it a draft again, which
    GitHub does not allow. The run therefore stops before the call.

    Parameters
    ----------
    state
        The release as this run's pre-edit read reported it.

    Returns
    -------
    PublishOutcome
        :attr:`PublishVerdict.VERIFIED` when the release is a draft, otherwise
        :attr:`PublishVerdict.NOT_A_DRAFT` with ``is_draft`` false.

    Examples
    --------
    >>> from finalize_github_release_state import ReleaseState
    >>> draft = ReleaseState("v1.2.3", True, False, {})
    >>> draft_gate(draft).verdict.value
    'verified'
    >>> public = ReleaseState("v1.2.3", False, False, {})
    >>> draft_gate(public).verdict.value
    'not-a-draft'
    """
    if state.is_draft:
        return PublishOutcome(PublishVerdict.VERIFIED)
    return PublishOutcome(PublishVerdict.NOT_A_DRAFT, is_draft=False)


def already_finalized() -> PublishOutcome:
    """Record a retry that found the release already public and matching.

    Returns
    -------
    PublishOutcome
        :attr:`PublishVerdict.ALREADY_FINALIZED` with ``is_draft`` false.

    Notes
    -----
    No edit is made on this path, so the outcome must not say ``finalized``.
    An interrupted retry converges instead of re-publishing, and the step
    reports which of the two ways the release became public.

    Examples
    --------
    >>> already_finalized().verdict.value
    'already-finalized'
    """
    return PublishOutcome(PublishVerdict.ALREADY_FINALIZED, is_draft=False)


def remote_failure(detail: str, *, after_edit: bool) -> PublishOutcome:
    """Report a call that failed without establishing the release's state.

    Parameters
    ----------
    detail
        The clause naming the command and what it answered.
    after_edit
        Whether an un-drafting call had already been attempted, which decides
        how much the diagnosis may claim.

    Returns
    -------
    PublishOutcome
        :attr:`PublishVerdict.REMOTE_FAILED` with the state left unknown.

    Examples
    --------
    >>> outcome = remote_failure("gh release edit exited 1", after_edit=True)
    >>> outcome.verdict.value, outcome.is_draft, outcome.after_edit
    ('remote-failed', None, True)
    """
    return PublishOutcome(
        PublishVerdict.REMOTE_FAILED,
        detail=detail,
        after_edit=after_edit,
    )
