"""Read the release and record what a finalization run observed.

This module holds the two read-only operations and the three records they and
the decision module write. They are separated from the decision itself because
their failure modes are the ones an operator reads first: a `gh` call that did
not answer, and a record that could not be written. Neither changes the
release, and every one of them stops the run with the release still a draft.

The split is also a size boundary. ``finalize_github_release_operations``
answers "may this release be un-drafted, and with which flag", and the module
here answers "what did GitHub say, and what did this run record". Keeping them
apart lets each stay small enough to read in one sitting.

Run via ``scripts/finalize_github_release.py``; behavioural coverage lives in
``scripts/tests/test_finalize_github_release_publish.py`` and
``scripts/tests/test_finalize_github_release.py``.
"""

import json
import sys
import typing as typ

import finalize_github_release_io as release_io
import finalize_github_release_state as release_state
import release_candidate
import release_identity_record

if typ.TYPE_CHECKING:
    # The path annotations are lazy on the Python 3.14 baseline and nothing
    # here reads them at runtime, so importing the module would only cost
    # start-up on a path that every finalization run takes.
    import pathlib


def snapshot_release(tag: str, repository: str) -> release_state.ReleaseState:
    """Read GitHub's current view of the release and decode it.

    Parameters
    ----------
    tag
        The release to read.
    repository
        The ``OWNER/REPO`` the release belongs to. A fork's release is not the
        upstream's, so it is never inferred from the checkout.

    Returns
    -------
    release_state.ReleaseState
        The draft flag, the prerelease flag, and each attached asset's reported
        digest.

    Notes
    -----
    A ``gh`` call that fails or answers with something unusable stops the run
    with :data:`EXIT_REMOTE_FAILED`; nothing is decided from an unread release.
    """
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
        report_remote_failure(result.exit_code, result.stderr, "gh release view")
    try:
        payload = json.loads(result.stdout or "")
        return release_state.parse_release_state(payload, tag=tag)
    except (json.JSONDecodeError, release_state.FinalizePolicyError) as error:
        release_io.fail(
            release_io.EXIT_REMOTE_FAILED,
            f"gh release view returned an unusable response: {error}",
        )


def report_remote_failure(exit_code: int, stderr: str | None, command: str) -> None:
    """Relay the command's own diagnosis, then fail with the remote status.

    Parameters
    ----------
    exit_code
        The status ``gh`` reported, which the diagnosis carries through.
    stderr
        Whatever ``gh`` wrote to standard error, relayed unchanged so its own
        wording reaches the log rather than a paraphrase.
    command
        The command as it was written, so the log names what failed.

    Notes
    -----
    The run stops with :data:`EXIT_REMOTE_FAILED`, and the message says the
    release remains a draft: a failed read or a failed edit changes nothing.
    """
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
    """Record the outcome atomically, then publish it to the workflow.

    Parameters
    ----------
    result_path
        Where the outcome record is written, before the workflow is told it.
    github_output
        The workflow's output file, or ``None`` when the step asked for none.
    outcome
        ``finalized`` or ``already-finalized``.
    identity
        The validated candidate, which supplies the tag and prerelease flag
        the record and the outputs carry.

    Notes
    -----
    The record is written first so a cancel between the two writes leaves
    evidence on disk. Either write failing stops the run with
    :data:`EXIT_OUTPUT_UNWRITABLE`; the release is already public by then, and
    the diagnosis says so.

    Examples
    --------
    >>> import pathlib
    >>> import tempfile
    >>> identity = release_candidate.ReleaseIdentity(
    ...     "v1.2.3", "0" * 40, "p", "1.2.3", "b", False
    ... )
    >>> with tempfile.TemporaryDirectory() as directory:
    ...     result = pathlib.Path(directory) / "result.json"
    ...     record_outcome(result, None, "finalized", identity)
    ...     json.loads(result.read_text(encoding="utf-8"))["prerelease"]
    release v1.2.3 is public (outcome finalized, prerelease False)
    False
    """
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

    Parameters
    ----------
    identity
        The package record ``package-crate`` retained.
    tag
        The release being finalized, which the record must name.
    commit
        The revision being published, which the record must name.
    output
        Where the candidate record is written for the later steps to read.

    Notes
    -----
    A record that is not a package record, holds no usable identity, or names
    another tag or revision stops the run with :data:`EXIT_INPUT_UNREADABLE`
    or :data:`EXIT_REFUSED`, before anything is written.

    The tag-and-revision agreement is :func:`release_candidate.identity_refusal`,
    the same rule ``publish`` applies to the candidate record. One rule read
    twice cannot report the same disagreement two ways.
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
    reason = release_candidate.identity_refusal(recorded, tag=tag, commit=commit)
    if reason is not None:
        release_io.fail(release_io.EXIT_REFUSED, reason)
    release_io.write_json(output, recorded.as_record())
    print(f"recorded candidate {recorded.tag} at {recorded.commit}")


def record_snapshot(
    tag: str,
    repository: str,
    *,
    state_path: pathlib.Path,
    assets_path: pathlib.Path,
) -> None:
    """Record the release's draft flag, prerelease flag, and reported digests.

    Parameters
    ----------
    tag
        The release to read.
    repository
        The ``OWNER/REPO`` the release belongs to.
    state_path
        Where the whole state record is written.
    assets_path
        Where the bare name-to-digest mapping is written for the planner, which
        has no business reading a draft flag.

    Notes
    -----
    A release that cannot be read stops the run with
    :data:`EXIT_REMOTE_FAILED`; a record that cannot be written stops it with
    :data:`EXIT_OUTPUT_UNWRITABLE`. This function only reads GitHub, so an
    interrupted snapshot never leaves the release in a different state.
    """
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
