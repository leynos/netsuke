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
"""

import dataclasses
import json

# ``PublishRequest`` reads these annotations from the class at runtime.
import pathlib  # ruff: ignore[typing-only-standard-library-import]
import sys

import finalize_github_release_io as release_io
import finalize_github_release_plan as release_plan
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

    Returns
    -------
    release_candidate.ReleaseIdentity
        The identity the package stage recorded.

    Examples
    --------
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
    _require_candidate(identity, tag=tag, commit=commit)
    return identity


def _require_candidate(
    identity: release_candidate.ReleaseIdentity, *, tag: str, commit: str
) -> None:
    """Stop unless the identity names this run's tag and revision."""
    if identity.tag is None:
        release_io.fail(
            release_io.EXIT_REFUSED,
            "the recorded candidate names no tag, so a rehearsal record "
            "cannot be finalized",
        )
    if identity.tag != tag:
        release_io.fail(
            release_io.EXIT_REFUSED,
            f"the recorded candidate names tag {identity.tag!r}, not {tag!r}; "
            "the release remains a draft",
        )
    if identity.commit != commit:
        release_io.fail(
            release_io.EXIT_REFUSED,
            f"the recorded candidate is commit {identity.commit}, not "
            f"{commit}; the release remains a draft",
        )


def assessment(path: pathlib.Path) -> release_plan.PlanAssessment:
    """Read the reconciliation plan and classify what it permits."""
    result = release_plan.assess_plan(release_io.read_json(path))
    if result.verdict is release_plan.PlanVerdict.UNREADABLE:
        release_io.fail(
            release_io.EXIT_INPUT_UNREADABLE,
            f"{path} is not a usable plan: {result.detail}",
        )
    return result


def require_verified(result: release_plan.PlanAssessment) -> None:
    """Stop unless the plan says every declared asset is already correct."""
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


def snapshot_release(tag: str, repository: str) -> release_state.ReleaseState:
    """Read GitHub's current view of the release and decode it."""
    result = release_io.run_gh(
        "release",
        "view",
        tag,
        "--repo",
        repository,
        "--json",
        release_io.VIEW_FIELDS,
    )
    if result.exit_code != 0:
        _report_remote_failure(result.exit_code, result.stderr, "gh release view")
    try:
        payload = json.loads(result.stdout or "")
        return release_state.parse_release_state(payload, tag=tag)
    except (json.JSONDecodeError, release_state.FinalizePolicyError) as error:
        release_io.fail(
            release_io.EXIT_REMOTE_FAILED,
            f"gh release view returned an unusable response: {error}",
        )


def _report_remote_failure(exit_code: int, stderr: str | None, command: str) -> None:
    """Relay the command's own diagnosis, then fail with the remote status."""
    if stderr:
        print(stderr, end="", file=sys.stderr)
    release_io.fail(
        release_io.EXIT_REMOTE_FAILED,
        f"{command} exited {exit_code}; the release remains a draft",
    )


def record_outcome(
    result_path: pathlib.Path,
    github_output: pathlib.Path | None,
    outcome: str,
    identity: release_candidate.ReleaseIdentity,
) -> None:
    """Record the outcome atomically, then publish it to the workflow."""
    release_io.write_json(
        result_path,
        {
            "outcome": outcome,
            "prerelease": identity.is_prerelease,
            "tag": identity.tag,
        },
        context="the release itself is already final, but its outcome was not recorded",
    )
    release_io.write_github_output(
        github_output, outcome, is_prerelease=identity.is_prerelease
    )
    print(
        f"release {identity.tag} is public (outcome {outcome}, "
        f"prerelease {identity.is_prerelease})"
    )


def record_candidate(
    identity: pathlib.Path, *, tag: str, commit: str, output: pathlib.Path
) -> None:
    """Check the package record's identity, then record the candidate.

    The package record nests the identity under ``identity`` beside the crate
    name and digest; only the identity is this stage's concern, and it is
    re-validated so a hand-edited record cannot reclassify the release.
    """
    envelope = release_io.read_json(identity)
    if not isinstance(envelope, dict) or not isinstance(envelope.get("identity"), dict):
        release_io.fail(
            release_io.EXIT_INPUT_UNREADABLE,
            f"{identity} is not a package record with an identity object",
        )
    try:
        recorded = release_identity_record.identity_from_mapping(envelope["identity"])
    except release_candidate.IdentityError as error:
        release_io.fail(
            release_io.EXIT_INPUT_UNREADABLE,
            f"{identity} is not a usable candidate: {error}",
        )
    if recorded.tag is None or recorded.tag != tag:
        release_io.fail(
            release_io.EXIT_REFUSED,
            f"the package record names tag {recorded.tag!r}, not {tag!r}",
        )
    if recorded.commit != commit:
        release_io.fail(
            release_io.EXIT_REFUSED,
            f"the package record is commit {recorded.commit}, not {commit}",
        )
    release_io.write_json(output, recorded.as_record())
    print(f"recorded candidate {recorded.tag} at {recorded.commit}")


def record_snapshot(
    tag: str,
    repository: str,
    *,
    state_path: pathlib.Path,
    assets_path: pathlib.Path,
) -> None:
    """Record the release's draft flag, prerelease flag, and reported digests."""
    state = snapshot_release(tag, repository)
    release_io.write_json(
        state_path,
        {
            "assets": state.assets,
            "isDraft": state.is_draft,
            "isPrerelease": state.is_prerelease,
            "tag": state.tag,
        },
    )
    release_io.write_json(assets_path, state.assets)
    print(
        f"release {tag}: draft={state.is_draft} "
        f"prerelease={state.is_prerelease} assets={len(state.assets)}"
    )


def publish_release(request: PublishRequest) -> None:
    """Un-draft a fully verified release, or report why it stays a draft."""
    identity = candidate(request.candidate_path, tag=request.tag, commit=request.commit)
    require_verified(assessment(request.plan_path))
    current = snapshot_release(request.tag, request.repository)
    if release_state.is_already_finalized(
        current, is_prerelease=identity.is_prerelease
    ):
        record_outcome(
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
        _report_remote_failure(edited.exit_code, edited.stderr, "gh release edit")
    readback = release_state.classify_readback(
        snapshot_release(request.tag, request.repository),
        is_prerelease=identity.is_prerelease,
    )
    match readback:
        case release_state.Readback.FINALIZED:
            record_outcome(
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
