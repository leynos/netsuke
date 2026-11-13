"""Read the release and record what a finalization run observed.

This module holds the reads and writes the run performs around the one edit:
the release snapshot, the candidate record the decision reads back, and the
outcome it records. They are separated from the decision itself because their
failure modes are the ones an operator reads first: a `gh` call that did not
answer, and a record that could not be written. Neither changes the release. A
failure before the edit reports only what this run established: that it has not
edited the release. A failure after one was attempted reports the state as
unknown, because GitHub may have applied the edit before the response was lost.

The split is also a size boundary. ``finalize_github_release_operations``
answers "may this release be un-drafted, and with which flag", and the module
here answers "what did GitHub say, and what did this run read or record". The
calls themselves are written in ``finalize_github_release_remote``, so this
module decodes their answers and records the outcome rather than assembling an
argument vector.
Keeping them apart lets each stay small enough to read in one sitting. The
writers for the two read-only stages — ``resolve``'s candidate record and
``snapshot``'s state record — live in
``finalize_github_release_stage_records``, because they run before the decision
and never touch the release.

Run via ``scripts/finalize_github_release.py``; behavioural coverage lives in
``scripts/tests/test_finalize_github_release_publish.py`` and
``scripts/tests/test_finalize_github_release.py``.
"""

import typing as typ

import finalize_github_release_io as release_io
import finalize_github_release_remote as release_remote
import release_candidate
import release_identity_record

if typ.TYPE_CHECKING:
    # These annotations are lazy on the Python 3.14 baseline and nothing here
    # reads them at runtime, so importing either would only cost start-up on a
    # path that every finalization run takes. The release state is decoded by
    # ``finalize_github_release_remote``, never constructed here.
    import pathlib

    import finalize_github_release_state as release_state


def snapshot_release(
    tag: str,
    repository: str,
    *,
    after_edit: bool = False,
    runner: release_io.Runner = release_io.run_gh,
) -> release_state.ReleaseState:
    """Read GitHub's current view of the release and decode it.

    Parameters
    ----------
    tag
        The release to read.
    repository
        The ``OWNER/REPO`` the release belongs to. A fork's release is not the
        upstream's, so it is never inferred from the checkout.
    after_edit
        Whether an un-drafting call has already been attempted for this run.
        A read that fails then cannot promise the release is unedited, because
        GitHub may have applied the edit before the response was lost.
    runner
        The process effect that answers the read. The command line supplies the
        Cuprum-backed one; a test supplies a function returning recorded
        answers, which is why no test needs a mocked process to exercise a
        decision.

    Returns
    -------
    release_state.ReleaseState
        The draft flag, the prerelease flag, and each attached asset's reported
        digest.

    Notes
    -----
    This is the raising wrapper around
    :func:`finalize_github_release_remote.read_release`, kept for the
    ``snapshot`` stage, which only records: a read that established nothing is
    a failure there rather than a decision. A ``FinalizeError`` from
    :func:`release_state_failure` propagates unchanged, with
    :data:`EXIT_REMOTE_FAILED`; nothing is decided from an unread release.
    """
    read = release_remote.read_release(tag, repository, runner=runner)
    if read.state is None:
        release_state_failure(read.detail, after_edit=after_edit)
    return read.state


def release_state_failure(message: str, *, after_edit: bool) -> typ.NoReturn:
    """Stop the run with a diagnosis that does not overstate what is known.

    Parameters
    ----------
    message
        What went wrong, as the caller would report it.
    after_edit
        Whether an un-drafting call has already been attempted for this run.

    Raises
    ------
    FinalizeError
        With :data:`EXIT_REMOTE_FAILED`, always.

    Notes
    -----
    A failed read establishes nothing about the release, so neither case
    reports it as a draft. A retry can begin after an earlier attempt already
    published the release, and this attempt's first read is exactly what
    failed; an operator told the release was still a draft might then edit it
    by hand. The sentence therefore comes from
    :func:`finalize_github_release_remote.unknown_state_message`, which the
    command layer uses for the same failure, so the two cannot drift apart.
    """
    raise release_io.FinalizeError(
        release_io.EXIT_REMOTE_FAILED,
        f"{message}. {release_remote.unknown_state_message(after_edit=after_edit)}",
    )


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

    Raises
    ------
    FinalizeError
        With :data:`EXIT_INPUT_UNREADABLE` when the record cannot be read or
        holds no usable identity, and :data:`EXIT_REFUSED` when it names
        another tag or revision, so no bytes from another candidate can be
        published under this release.

    Examples
    --------
    >>> import json
    >>> import pathlib
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
        raise release_io.FinalizeError(
            release_io.EXIT_INPUT_UNREADABLE, f"{path} does not hold a JSON object"
        )
    try:
        identity = release_identity_record.identity_from_mapping(decoded)
    except release_candidate.IdentityError as error:
        raise release_io.FinalizeError(
            release_io.EXIT_INPUT_UNREADABLE,
            f"{path} is not a usable candidate: {error}",
        ) from error
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

    Raises
    ------
    FinalizeError
        With :data:`EXIT_REFUSED` when the identity names another tag or
        revision. The mismatch is detected before any API call, so this run
        cannot have edited the release; the release's own state is not read
        here, and a retry may begin after an earlier attempt published it, so
        the diagnosis claims no more than that.

    Notes
    -----
    The agreement itself is :func:`release_candidate.identity_refusal`, so the
    package record ``resolve`` checks and the candidate record this module
    checks cannot disagree about what counts as this run's candidate.
    """
    reason = release_candidate.identity_refusal(identity, tag=tag, commit=commit)
    if reason is not None:
        raise release_io.FinalizeError(
            release_io.EXIT_REFUSED, f"{reason}; this run has not edited the release"
        )


def record_outcome(
    result_path: pathlib.Path,
    github_output: pathlib.Path | None,
    outcome: str,
    identity: release_candidate.ReleaseIdentity,
) -> str:
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

    Returns
    -------
    str
        The sentence an operator should read now that the release is public.
        Returning it rather than printing it keeps the decision about output in
        the command layer.

    Notes
    -----
    A ``FinalizeError`` from either write propagates unchanged, with
    :data:`EXIT_OUTPUT_UNWRITABLE`; the release is already public by then, and
    the diagnosis says so.

    The record is written first so a cancel between the two writes leaves
    evidence on disk.

    Examples
    --------
    >>> import json
    >>> import pathlib
    >>> import tempfile
    >>> identity = release_candidate.ReleaseIdentity(
    ...     "v1.2.3", "0" * 40, "p", "1.2.3", "b", False
    ... )
    >>> with tempfile.TemporaryDirectory() as directory:
    ...     result = pathlib.Path(directory) / "result.json"
    ...     print(record_outcome(result, None, "finalized", identity))
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
    return (
        f"release {identity.tag} is public (outcome {outcome}, "
        f"prerelease {identity.is_prerelease})"
    )
