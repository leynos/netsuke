"""Decide and perform the two operations a publishing run is built from.

Un-drafting a release is irreversible, so the guards here run before any API
call: a candidate built from another revision, a plan that still owes an
upload, and a plan whose bytes were never established each stop the run before
it touches the release. Only a release in which every declared asset already
holds the trusted digest reaches the one edit this feature performs.

The plan answers for the snapshot the planner read, so the release is
reconciled a second time against the trusted record immediately before the
edit. That second pass is the planner's own policy, not a cheaper copy of it:
a draft that lost or replaced an asset between the two reads is judged exactly
as a first read would judge it.

The plan is read from the file ``finalize_github_release_cli`` wrote, and the
candidate from the record ``package-crate`` wrote. The inventory verifier's
output is deliberately not an input: comparing GitHub against a record derived
from GitHub would agree with itself however the upload was tampered with.

Run via ``scripts/finalize_github_release.py``; behavioural coverage lives in
``scripts/tests/test_finalize_github_release_publish.py`` and
``scripts/tests/test_finalize_github_release_publish_failures.py``.

What GitHub answered, what this run recorded, and the candidate record it read
live in ``finalize_github_release_records``. This module decides from those
records; it does not produce them.
"""

import dataclasses

# ``PublishRequest`` reads these annotations from the class at runtime.
import pathlib  # ruff: ignore[typing-only-standard-library-import]
import typing as typ

import finalize_github_release_io as release_io
import finalize_github_release_plan as release_plan
import finalize_github_release_records as release_records
import finalize_github_release_state as release_state
import release_digest_record
import release_digest_record_files
import release_github_assets_adapter
import release_reconcile_github
import release_upload_plan

if typ.TYPE_CHECKING:
    # Only the helper signatures name the identity; nothing here constructs one.
    import release_candidate


@dataclasses.dataclass(frozen=True, slots=True)
class PublishRequest:
    """Name every input and output one ``publish`` run works with.

    Attributes
    ----------
    tag
        The release the workflow is finalizing.
    commit
        The revision the candidate must have been built from.
    repository
        The ``OWNER/REPO`` the release belongs to.
    candidate_path
        The candidate record ``resolve`` wrote.
    plan_path
        The reconciliation plan ``finalize_github_release_cli`` wrote.
    digest_record_path
        The trusted digest record the declaration stage wrote before upload.
        Re-read here so the release is reconciled against the same record the
        planner used, not against the plan's restatement of it.
    result_path
        Where the outcome record is written.
    github_output
        The workflow's output file, when the step asked for outputs.
    """

    tag: str
    commit: str
    repository: str
    candidate_path: pathlib.Path
    plan_path: pathlib.Path
    digest_record_path: pathlib.Path
    result_path: pathlib.Path
    github_output: pathlib.Path | None = None


def assessment(path: pathlib.Path) -> release_plan.PlanAssessment:
    """Read the reconciliation plan and classify what it permits.

    Parameters
    ----------
    path
        The plan ``finalize_github_release_cli`` wrote for this run.

    Returns
    -------
    release_plan.PlanAssessment
        The verdict, the assets each verdict rests on, and the reason a
        refused plan gives.

    Notes
    -----
    A plan whose shape supports no decision stops the run with
    :data:`EXIT_INPUT_UNREADABLE` instead of being read as permission to
    proceed.
    """
    result = release_plan.assess_plan(release_io.read_json(path))
    if result.verdict is release_plan.PlanVerdict.UNREADABLE:
        release_io.fail(
            release_io.EXIT_INPUT_UNREADABLE,
            f"{path} is not a usable plan: {result.detail}",
        )
    return result


def require_verified(
    result: release_plan.PlanAssessment,
    *,
    source: str = "the reconciliation plan",
    draft_confirmed: bool = False,
) -> None:
    """Stop unless the assessment says every declared asset is already correct.

    Parameters
    ----------
    result
        The assessment :func:`assessment` or :func:`_reconcile_current`
        produced.
    source
        What the assessment was made from, named in the refusal so an operator
        can tell a stale plan from a release that changed under the run.
    draft_confirmed
        Whether a read taken for this run established that the release is
        still a draft. Only then may the refusal say so.

    Notes
    -----
    Only a verified assessment returns. One that still owes an upload stops
    the run with :data:`EXIT_UPLOAD_REQUIRED`, pointing at the declared upload
    contract; a conflicting or unverifiable one stops it with
    :data:`EXIT_REFUSED`; an unreadable one with :data:`EXIT_INPUT_UNREADABLE`.
    Each stops the run before the edit, so each can say this run has not
    edited the release. A refusal made from the plan alone has read nothing,
    and a retry may begin after an earlier attempt published the release, so
    only an assessment made after :func:`_require_draft` passes
    ``draft_confirmed``.

    Examples
    --------
    >>> from finalize_github_release_plan import PlanAssessment, PlanVerdict
    >>> require_verified(PlanAssessment(PlanVerdict.VERIFIED))
    >>> try:
    ...     require_verified(
    ...         PlanAssessment(PlanVerdict.REFUSED, conflicts=("a.tar.gz",))
    ...     )
    ... except SystemExit as refusal:
    ...     refusal.code
    1
    """
    if result.verdict is release_plan.PlanVerdict.VERIFIED:
        return
    state = _draft_state(draft_confirmed=draft_confirmed)
    if result.verdict is release_plan.PlanVerdict.UPLOAD_REQUIRED:
        release_io.fail(
            release_io.EXIT_UPLOAD_REQUIRED,
            f"{source} still requires uploads ("
            + ", ".join(result.uploads)
            + "); run the declared upload contract before finalizing; "
            + state,
        )
    if result.verdict is release_plan.PlanVerdict.UNREADABLE:
        release_io.fail(release_io.EXIT_INPUT_UNREADABLE, result.detail)
    reason = (
        release_upload_plan.blocked_message(result.conflicts, result.unverifiable)
        or result.detail
    )
    release_io.fail(release_io.EXIT_REFUSED, f"{reason}; {state}")


def _draft_state(*, draft_confirmed: bool) -> str:
    """Name the release state only as far as this run has established it.

    A refusal made from the plan alone has not read the release, and a retry
    can begin after an earlier attempt already published it, so it may not
    claim the release is a draft. A refusal made after the pre-edit read may,
    because that read saw ``isDraft: true``.

    Returns
    -------
    str
        The clause a refusal appends, claiming a draft only when a read
        established one.
    """
    if draft_confirmed:
        return "the release remains a draft"
    return "this run has not edited the release"


def publish_release(request: PublishRequest) -> None:
    """Un-draft a fully verified release, or report why it is left unedited.

    Parameters
    ----------
    request
        The tag, revision, repository, and record locations for this run.

    Notes
    -----
    Every guard runs before the single mutating call. A candidate from another
    revision, a plan that still owes an upload, a conflict, or a public
    release with the wrong prerelease flag stops the run with
    :data:`EXIT_REFUSED`, :data:`EXIT_UPLOAD_REQUIRED`, or
    :data:`EXIT_INPUT_UNREADABLE`, leaving the release unedited by this run. A
    failed call or an unreadable readback stops it with
    :data:`EXIT_REMOTE_FAILED`, and an outcome that cannot be recorded with
    :data:`EXIT_OUTPUT_UNWRITABLE` after the edit has landed. Those two are the
    only failures that cannot promise the release is unedited: once the edit
    has been attempted, GitHub may have applied it before the response was
    lost, so the run reports the state as unknown and names the two ways to
    establish it.

    A release already public with the candidate's own flag is recorded as
    ``already-finalized`` without a second edit, so an interrupted retry
    converges instead of re-publishing.

    The plan answers for the snapshot the planner read, not for the release as
    it is at the edit. So the fresh snapshot is reconciled again against the
    same trusted record before the call: a declared asset that vanished is an
    :data:`EXIT_UPLOAD_REQUIRED` refusal, one whose bytes changed is an
    :data:`EXIT_REFUSED` refusal, and an undeclared extra is still ignored
    exactly as the planner ignores it. A record that cannot be read stops the
    run with :data:`EXIT_INPUT_UNREADABLE`. Every one of those leaves the
    release unedited, and each is reached after a read that saw it is still a
    draft, so each may say so.

    What remains, and cannot be closed from this side, is the window between
    that read and the edit: GitHub offers no conditional edit, so a writer that
    changes the draft in those milliseconds is not observed. The window is one
    API call wide and both endpoints are the trusted record, so a run that
    loses the race is detected on the next reconciliation rather than silently
    published over.
    """
    identity = release_records.candidate(
        request.candidate_path, tag=request.tag, commit=request.commit
    )
    require_verified(assessment(request.plan_path))
    current = release_records.snapshot_release(request.tag, request.repository)
    if release_state.is_already_finalized(
        current, is_prerelease=identity.is_prerelease
    ):
        release_records.record_outcome(
            request.result_path, request.github_output, "already-finalized", identity
        )
        return
    _require_draft(current)
    require_verified(
        _reconcile_current(request.digest_record_path, current),
        source="the release's current inventory",
        draft_confirmed=True,
    )
    _edit_release(request, identity)
    _confirm_published(request, identity)


def _edit_release(
    request: PublishRequest, identity: release_candidate.ReleaseIdentity
) -> None:
    """Un-draft the release with the candidate's own prerelease flag.

    Notes
    -----
    This is the only mutating call the feature makes. A failure stops the run
    with :data:`EXIT_REMOTE_FAILED` and reports the state as unknown, because
    GitHub may have applied the edit before the response was lost.
    """
    edited = release_io.run_gh(
        "release",
        "edit",
        request.tag,
        "--repo",
        request.repository,
        "--draft=false",
        f"--prerelease={'true' if identity.is_prerelease else 'false'}",
        echo=True,
    )
    if edited.exit_code != 0:
        release_records.report_remote_failure(
            edited.exit_code, edited.stderr, "gh release edit", after_edit=True
        )


def _confirm_published(
    request: PublishRequest, identity: release_candidate.ReleaseIdentity
) -> None:
    """Read the release back and record the outcome that read supports.

    Notes
    -----
    A read-back that cannot be taken stops the run with
    :data:`EXIT_REMOTE_FAILED` and reports the state as unknown. A release that
    is still a draft, or public with the wrong prerelease flag, is a failure
    the run cannot repair: GitHub does not allow a public release to become a
    draft again.
    """
    readback = release_state.classify_readback(
        release_records.snapshot_release(
            request.tag, request.repository, after_edit=True
        ),
        is_prerelease=identity.is_prerelease,
    )
    match readback:
        case release_state.Readback.FINALIZED:
            release_records.record_outcome(
                request.result_path, request.github_output, "finalized", identity
            )
        case release_state.Readback.STILL_DRAFT:
            release_io.fail(
                release_io.EXIT_REMOTE_FAILED,
                "gh accepted the edit but the release is still a draft; no "
                "asset was replaced and the release was not published",
            )
        case release_state.Readback.PRERELEASE_MISMATCH:
            release_io.fail(
                release_io.EXIT_REFUSED,
                "the release is public but its prerelease state contradicts "
                "the validated candidate; it cannot be made a draft again",
            )


def _reconcile_current(
    digest_record_path: pathlib.Path, state: release_state.ReleaseState
) -> release_plan.PlanAssessment:
    """Re-run the planner's reconciliation against the release as it is now.

    Parameters
    ----------
    digest_record_path
        The trusted digest record the declaration stage wrote before upload.
    state
        The release as the pre-edit snapshot reported it.

    Returns
    -------
    release_plan.PlanAssessment
        The verdict the planner would have reached for this listing.

    Notes
    -----
    The record is re-read rather than taken from the plan, so the comparison is
    against the bytes the build declared and not against a document the
    planner wrote. The digest conversion is
    :func:`release_github_assets_adapter.normalise_github_digest`, the same one
    the planner uses, so an unusable digest is unverifiable here for the same
    reason it was there.

    A record that cannot be read stops the run with
    :data:`EXIT_INPUT_UNREADABLE`: an absent record would plan no actions, and
    no actions is the shape of a fully verified release.
    """
    try:
        expected = release_digest_record_files.read_digest_record(digest_record_path)
    except release_digest_record.DigestRecordError as error:
        release_io.fail(
            release_io.EXIT_INPUT_UNREADABLE,
            f"{digest_record_path} is not a usable digest record: {error}",
        )
    present = {
        name: release_github_assets_adapter.normalise_github_digest(digest)
        for name, digest in state.assets.items()
    }
    return release_plan.assess_actions(
        release_reconcile_github.plan_asset_actions(expected=expected, present=present)
    )


def _require_draft(state: release_state.ReleaseState) -> None:
    """Refuse to edit a release that has already been published.

    A public release has nothing to un-draft, and one with the wrong
    prerelease flag cannot be corrected without making it a draft again, which
    GitHub does not allow. The run therefore stops before the call rather than
    acting on a state it must not touch.
    """
    if state.is_draft:
        return
    release_io.fail(
        release_io.EXIT_REFUSED,
        "the release is public but its prerelease state contradicts the "
        "validated candidate; a public release cannot be made a draft again",
    )
