"""Decide and perform the three operations a finalization run is built from.

Un-drafting a release is irreversible, so the guards here run before any API
call: a candidate built from another revision, a plan that still owes an
upload, and a plan whose bytes were never established each stop the run with
the release still a draft. Only a plan in which every declared asset already
holds the trusted digest reaches the one edit this feature performs.

The plan is read from the file ``finalize_github_release_cli`` wrote, and the
candidate from the record ``package-crate`` wrote. The inventory verifier's
output is deliberately not an input: comparing GitHub against a record derived
from GitHub would agree with itself however the upload was tampered with.

Run via ``scripts/finalize_github_release.py``; behavioural coverage lives in
``scripts/tests/test_finalize_github_release.py`` and
``scripts/tests/test_finalize_github_release_publish.py``.

What GitHub answered and what this run recorded live in
``finalize_github_release_records``. This module decides from those records; it
does not produce them.
"""

import dataclasses

# ``PublishRequest`` reads these annotations from the class at runtime.
import pathlib  # ruff: ignore[typing-only-standard-library-import]

import finalize_github_release_io as release_io
import finalize_github_release_plan as release_plan
import finalize_github_release_records as release_records
import finalize_github_release_state as release_state
import release_candidate
import release_identity_record
import release_upload_plan


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
    result_path: pathlib.Path
    github_output: pathlib.Path | None = None


def candidate(
    path: pathlib.Path, *, tag: str, commit: str
) -> release_candidate.ReleaseIdentity:
    """Read the recorded candidate and require it to name this run's revision.

    Parameters
    ----------
    path
        The candidate record ``resolve`` wrote for this run.
    tag
        The release being finalized, which the record must name.
    commit
        The revision being published, which the record must name.

    Returns
    -------
    release_candidate.ReleaseIdentity
        The identity the package stage recorded.

    Notes
    -----
    A record that cannot be read, does not hold a usable identity, or names
    another tag or revision stops the run with :data:`EXIT_INPUT_UNREADABLE`
    or :data:`EXIT_REFUSED`, so no bytes from another candidate can be
    published under this release.

    Examples
    --------
    >>> import json
    >>> import tempfile
    >>> record = {"tag": "v1.2.3", "commit": "0" * 40,
    ...           "package_name": "p", "version": "1.2.3",
    ...           "bin_name": "b", "is_prerelease": False}
    >>> with tempfile.TemporaryDirectory() as directory:
    ...     source = pathlib.Path(directory) / "candidate.json"
    ...     _ = source.write_text(json.dumps(record), encoding="utf-8")
    ...     candidate(source, tag="v1.2.3", commit="0" * 40).version
    '1.2.3'
    """
    decoded = release_io.read_json(path)
    if not isinstance(decoded, dict):
        release_io.fail(
            release_io.EXIT_INPUT_UNREADABLE, f"{path} does not hold a JSON object"
        )
    try:
        identity = release_identity_record.identity_from_mapping(decoded)
    except release_candidate.IdentityError as error:
        release_io.fail(
            release_io.EXIT_INPUT_UNREADABLE,
            f"{path} is not a usable candidate: {error}",
        )
    require_candidate(identity, tag=tag, commit=commit)
    return identity


def require_candidate(
    identity: release_candidate.ReleaseIdentity, *, tag: str, commit: str
) -> None:
    """Stop unless the identity names this run's tag and revision.

    Parameters
    ----------
    identity
        The identity an earlier stage recorded.
    tag
        The release this run is finalizing.
    commit
        The revision being published.

    Notes
    -----
    The agreement itself is :func:`release_candidate.identity_refusal`, so the
    package record ``resolve`` checks and the candidate record this module
    checks cannot disagree about what counts as this run's candidate. A
    mismatch stops the run with :data:`EXIT_REFUSED` before any API call, so
    the release remains a draft.
    """
    reason = release_candidate.identity_refusal(identity, tag=tag, commit=commit)
    if reason is not None:
        release_io.fail(
            release_io.EXIT_REFUSED, f"{reason}; the release remains a draft"
        )


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


def require_verified(result: release_plan.PlanAssessment) -> None:
    """Stop unless the plan says every declared asset is already correct.

    Parameters
    ----------
    result
        The plan assessment :func:`assessment` produced.

    Notes
    -----
    Only a verified plan returns. A plan that still owes an upload stops the
    run with :data:`EXIT_UPLOAD_REQUIRED`, pointing at the declared upload
    contract; a conflicting or unverifiable plan stops it with
    :data:`EXIT_REFUSED`; an unreadable one with :data:`EXIT_INPUT_UNREADABLE`.
    Each leaves the release a draft.
    """
    match result.verdict:
        case release_plan.PlanVerdict.VERIFIED:
            return
        case release_plan.PlanVerdict.UPLOAD_REQUIRED:
            release_io.fail(
                release_io.EXIT_UPLOAD_REQUIRED,
                "the plan still requires uploads ("
                + ", ".join(result.uploads)
                + "); run the declared upload contract before finalizing. "
                "The release remains a draft",
            )
        case release_plan.PlanVerdict.REFUSED:
            reason = (
                release_upload_plan.blocked_message(
                    result.conflicts, result.unverifiable
                )
                or result.detail
            )
            release_io.fail(
                release_io.EXIT_REFUSED, f"{reason}; the release remains a draft"
            )
        case release_plan.PlanVerdict.UNREADABLE:
            release_io.fail(release_io.EXIT_INPUT_UNREADABLE, result.detail)


def publish_release(request: PublishRequest) -> None:
    """Un-draft a fully verified release, or report why it stays a draft.

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
    :data:`EXIT_INPUT_UNREADABLE`, leaving the release a draft. A failed call
    or an unreadable readback stops it with :data:`EXIT_REMOTE_FAILED`, and an
    outcome that cannot be recorded with :data:`EXIT_OUTPUT_UNWRITABLE` after
    the edit has landed.

    A release already public with the candidate's own flag is recorded as
    ``already-finalized`` without a second edit, so an interrupted retry
    converges instead of re-publishing.
    """
    identity = candidate(request.candidate_path, tag=request.tag, commit=request.commit)
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
            edited.exit_code, edited.stderr, "gh release edit"
        )
    readback = release_state.classify_readback(
        release_records.snapshot_release(request.tag, request.repository),
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


def _require_draft(state: release_state.ReleaseState) -> None:
    """Refuse to edit a release that has already been published.

    ``is_already_finalized`` has taken the case where the release is public
    with the candidate's flag, so a public release reaching here has the other
    one. Editing it would change a release someone may already have downloaded,
    and a public release cannot be made a draft again, so the run stops before
    the call rather than correcting a state it must not touch.
    """
    if state.is_draft:
        return
    release_io.fail(
        release_io.EXIT_REFUSED,
        "the release is public but its prerelease state contradicts the "
        "validated candidate; a public release cannot be made a draft again",
    )
