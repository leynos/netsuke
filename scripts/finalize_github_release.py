#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# dependencies = ["cyclopts==4.25.3", "cuprum==0.1.0"]
# [tool.uv]
# exclude-newer = "2026-11-10T00:00:00Z"
# ///
"""Finalize a verified GitHub release without ever replacing its bytes.

Un-drafting a release is irreversible, so this adapter performs exactly the
two calls that decision needs and nothing more: it reads the release, and it
un-drafts it when the recorded plan says every declared asset already holds
the trusted digest. It never uploads, deletes, or clobbers an asset; a missing
or conflicting asset is a refusal that leaves the release as a draft.

Both properties are enforced below the command line, in
``finalize_github_release_operations`` and ``finalize_github_release_remote``;
this module names the three operations the composite action invokes, the
options each one takes, and the words an operator reads. Reading the input
records, recording the outcome, mapping a verdict to an exit status, and
composing the sentence each stage prints all happen here, because those are the
only steps that belong to the process edge. It is also the one place a process
is bound: the Cuprum-backed runner is supplied here and nowhere below, so every
helper underneath takes the effect it uses as a required argument. The command
set is therefore part of the action's contract, and the example below is what
the action's coverage asserts:

>>> sorted(name for name in app.resolved_commands() if not name.startswith("-"))
['publish', 'resolve', 'snapshot']

Exit statuses are the vocabulary the workflow branches on:

``0``
    The release is public with the candidate's prerelease state.
``1``
    The plan or candidate refuses publication, the draft carries an attachment
    the trusted record never declared, or the release contradicts the
    candidate: before the edit when it is already public with the wrong flag,
    and after one when the read-back disagrees.
``2``
    The plan still requires an upload, which this stage never performs.
``3``
    An input record could not be read or decoded.
``4``
    A result could not be recorded.
``5``
    A GitHub call failed or answered with something unusable.

:class:`finalize_github_release_io.FinalizeError` carries the status of a
failure in the input and output effects, and :func:`main` turns it into a
process exit. A publishing decision travels as a
:class:`finalize_github_release_decisions.PublishOutcome` instead, which
:func:`finalize_github_release_messages.describe` renders and
:func:`_exit_status` maps. Keeping those two apart means changing how a
refusal reads cannot change what the step does.

Run via ``.github/actions/finalize-github-release``; behavioural coverage
lives in ``scripts/tests/test_finalize_github_release.py``.
"""

import dataclasses

# Cyclopts resolves these annotations when it builds the command, so the
# module needs ``pathlib`` at runtime rather than only for type checking.
import pathlib  # ruff: ignore[typing-only-standard-library-import]
import sys
import typing as typ

import finalize_github_release_decisions as decisions
import finalize_github_release_io as release_io
import finalize_github_release_messages as messages
import finalize_github_release_operations as operations
import finalize_github_release_records as release_records
import finalize_github_release_remote as release_remote
import finalize_github_release_stage_records as stage_records

from cyclopts import App, Parameter

if typ.TYPE_CHECKING:
    # Only :func:`main`'s signature names the sequence, and that value is
    # never inspected at run time, so the import costs a start-up it earns
    # nothing from.
    import collections.abc as cabc

app = App(help="Reconcile and un-draft a verified GitHub release.")

#: The verdicts that mean the release is now public, and so whose outcome is
#: recorded rather than reported as a refusal.
_PUBLIC = frozenset({
    decisions.PublishVerdict.FINALIZED,
    decisions.PublishVerdict.ALREADY_FINALIZED,
})

#: The status each verdict reports. A decision names what happened; this table
#: is the one place that spelling becomes the workflow's vocabulary.
_STATUS = {
    decisions.PublishVerdict.FINALIZED: release_io.EXIT_OK,
    decisions.PublishVerdict.ALREADY_FINALIZED: release_io.EXIT_OK,
    decisions.PublishVerdict.UPLOAD_REQUIRED: release_io.EXIT_UPLOAD_REQUIRED,
    decisions.PublishVerdict.INPUT_UNREADABLE: release_io.EXIT_INPUT_UNREADABLE,
    decisions.PublishVerdict.REMOTE_FAILED: release_io.EXIT_REMOTE_FAILED,
    decisions.PublishVerdict.STILL_DRAFT: release_io.EXIT_REMOTE_FAILED,
    decisions.PublishVerdict.REFUSED: release_io.EXIT_REFUSED,
    decisions.PublishVerdict.UNDECLARED_ASSET: release_io.EXIT_REFUSED,
    decisions.PublishVerdict.NOT_A_DRAFT: release_io.EXIT_REFUSED,
    decisions.PublishVerdict.PRERELEASE_MISMATCH: release_io.EXIT_REFUSED,
    decisions.PublishVerdict.PUBLISHED_MISMATCH: release_io.EXIT_REFUSED,
    decisions.PublishVerdict.VERIFIED: release_io.EXIT_OK,
}


@app.command
def resolve(
    *,
    identity: typ.Annotated[pathlib.Path, Parameter(required=True)],
    tag: typ.Annotated[str, Parameter(required=True)],
    commit: typ.Annotated[str, Parameter(required=True)],
    output: typ.Annotated[pathlib.Path, Parameter(required=True)],
) -> None:
    """Record the candidate the package stage built, after checking its identity.

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
    The ``FinalizeError`` that
    :func:`finalize_github_release_stage_records.record_candidate` raises
    propagates unchanged, because the status it carries is the one this step
    reports: ``3`` when the record cannot be read or decoded, and ``1`` when it
    names another tag or revision. Nothing is written in either case.
    """
    recorded = stage_records.record_candidate(
        identity, tag=tag, commit=commit, output=output
    )
    print(f"recorded candidate {recorded.tag} at {recorded.commit}")


@app.command
def snapshot(
    *,
    tag: typ.Annotated[str, Parameter(required=True)],
    repository: typ.Annotated[str, Parameter(required=True)],
    state_path: typ.Annotated[pathlib.Path, Parameter(required=True)],
    assets_path: typ.Annotated[pathlib.Path, Parameter(required=True)],
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
        Where the bare name-to-digest mapping is written for the planner.

    Notes
    -----
    The ``FinalizeError`` that
    :func:`finalize_github_release_stage_records.record_snapshot` raises
    propagates unchanged: ``5`` when GitHub cannot be read, and ``4`` when
    either record cannot be written. This command only reads GitHub, so a
    failure never changes the release.
    """
    state = stage_records.record_snapshot(
        tag,
        repository,
        paths=stage_records.SnapshotPaths(state=state_path, assets=assets_path),
        runner=release_io.run_gh,
    )
    print(
        f"release {tag}: draft={state.is_draft} "
        f"prerelease={state.is_prerelease} assets={len(state.assets)}"
    )


@dataclasses.dataclass(frozen=True, slots=True)
class PublishRequest:
    """Name the inputs and outputs one ``publish`` invocation works with.

    The paths live here, at the process edge, and nowhere below it: the
    decision takes the decoded records these paths name, so it never needs to
    know that a file was involved.

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


# `Parameter(name="*")` flattens the request into one flag per field, so the
# command line stays `--tag`, `--commit`, `--candidate-path` and so on while
# the command itself takes one grouped value. Fields without a default stay
# required, which is what the action's contract asserts.
@app.command
def publish(
    request: typ.Annotated[PublishRequest, Parameter(name="*")],
) -> None:
    """Un-draft a fully verified release, or report why it is left unedited.

    Parameters
    ----------
    request
        The tag, revision, repository, and record locations for this run. Each
        field is a flag of its own, so the command line stays ``--tag``,
        ``--commit``, ``--candidate-path`` and so on.

    Raises
    ------
    SystemExit
        With the status the outcome names, after its diagnosis reaches
        standard error, when the release was not made public by this run.

    Notes
    -----
    The three input records are read here, so a record that cannot be decoded
    stops the run before any API call and reports the status its own reader
    carries. The decision itself is made by
    :func:`finalize_github_release_operations.publish_release` from the decoded
    values, and this command either records the public outcome or renders the
    refusal.
    """
    identity = release_records.candidate(
        request.candidate_path, tag=request.tag, commit=request.commit
    )
    trusted = release_io.load_digest_record(request.digest_record_path)
    plan_document = release_io.read_json(request.plan_path)
    outcome = operations.publish_release(
        operations.PublishInputs(
            identity=identity,
            trusted=trusted,
            plan_document=plan_document,
            reader=release_remote.reader_port(
                identity.tag, request.repository, runner=release_io.run_gh
            ),
            editor=release_remote.editor_port(
                identity.tag, request.repository, runner=release_io.run_gh
            ),
        )
    )
    if outcome.verdict in _PUBLIC:
        release_records.record_outcome(
            request.result_path,
            request.github_output,
            outcome.verdict.value,
            identity,
        )
        print(
            f"release {identity.tag} is public (outcome {outcome.verdict.value}, "
            f"prerelease {identity.is_prerelease})"
        )
        return
    # ``release_records.candidate`` has already refused a record that names no
    # tag or another one, so the invocation's tag and the candidate's are the
    # same string here; naming the request keeps that equality explicit rather
    # than asking the reader to remember it.
    print(
        messages.describe(
            outcome,
            plan_path=request.plan_path,
            tag=request.tag,
            is_prerelease=identity.is_prerelease,
        ),
        file=sys.stderr,
    )
    raise SystemExit(_exit_status(outcome))


def _exit_status(outcome: decisions.PublishOutcome) -> int:
    """Map a verdict to the status the workflow step reports.

    Parameters
    ----------
    outcome
        The verdict being reported.

    Returns
    -------
    int
        The status from :mod:`finalize_github_release_io`'s exit vocabulary.

    Examples
    --------
    >>> _exit_status(decisions.PublishOutcome(decisions.PublishVerdict.STILL_DRAFT))
    5
    """
    return _STATUS[outcome.verdict]


def main(argv: cabc.Sequence[str] | None = None) -> None:
    """Run one finalizer command, mapping a refusal to the workflow's status.

    Parameters
    ----------
    argv
        The command line after the program name, or ``None`` to read the
        process's own arguments, as the workflow step invokes it.

    Raises
    ------
    SystemExit
        With the status the refusal carries, after its diagnosis reaches
        standard error. The effects raise :class:`FinalizeError` instead of
        exiting, and :func:`publish` raises this for a decision, so this is the
        one place the exit vocabulary becomes a process status.

    Examples
    --------
    >>> import tempfile
    >>> with tempfile.TemporaryDirectory() as directory:
    ...     source = pathlib.Path(directory) / "identity.json"
    ...     _ = source.write_text("{}", encoding="utf-8")
    ...     try:
    ...         main([
    ...             "resolve", "--identity", str(source), "--tag", "v1.2.3",
    ...             "--commit", "0" * 40,
    ...             "--output", str(pathlib.Path(directory) / "candidate.json"),
    ...         ])
    ...     except SystemExit as refusal:
    ...         refusal.code
    3
    """
    try:
        app(argv, result_action="return_value")
    except release_io.FinalizeError as failure:
        print(failure, file=sys.stderr)
        raise SystemExit(failure.code) from None


if __name__ == "__main__":
    main()
