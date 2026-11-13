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
``finalize_github_release_operations`` and
``finalize_github_release_records``; this module only names the three
operations the composite action invokes and the options each one takes. The
command set is therefore part of the action's contract, and the example below
is what the action's coverage asserts:

>>> sorted(name for name in app.resolved_commands() if not name.startswith("-"))
['publish', 'resolve', 'snapshot']

Exit statuses are the vocabulary the workflow branches on:

``0``
    The release is public with the candidate's prerelease state.
``1``
    The plan or candidate refuses publication, or the release contradicts the
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

Each helper below raises :class:`finalize_github_release_io.FinalizeError`
carrying one of those statuses, and :func:`main` is the only place a status
becomes a process exit.

Run via ``.github/actions/finalize-github-release``; behavioural coverage
lives in ``scripts/tests/test_finalize_github_release.py``.
"""

# Cyclopts resolves these annotations when it builds the command, so the
# module needs ``pathlib`` at runtime rather than only for type checking.
import pathlib  # ruff: ignore[typing-only-standard-library-import]
import sys
import typing as typ

import finalize_github_release_io as release_io
import finalize_github_release_operations as operations
import finalize_github_release_stage_records as stage_records

from cyclopts import App, Parameter

if typ.TYPE_CHECKING:
    # Only :func:`main`'s signature names the sequence, and that value is
    # never inspected at run time, so the import costs a start-up it earns
    # nothing from.
    import collections.abc as cabc

app = App(help="Reconcile and un-draft a verified GitHub release.")


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
    :func:`finalize_github_release_stage_records.record_candidate` raises propagates
    unchanged, because the status it carries is the one this step reports:
    ``3`` when the record cannot be read or decoded, and ``1`` when it names
    another tag or revision. Nothing is written in either case.
    """
    stage_records.record_candidate(identity, tag=tag, commit=commit, output=output)


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
    :func:`finalize_github_release_stage_records.record_snapshot` raises propagates
    unchanged: ``5`` when GitHub cannot be read, and ``4`` when either record
    cannot be written. This command only reads GitHub, so a failure never
    changes the release.
    """
    stage_records.record_snapshot(
        tag, repository, state_path=state_path, assets_path=assets_path
    )


# `Parameter(name="*")` flattens the request into one flag per field, so the
# command line stays `--tag`, `--commit`, `--candidate-path` and so on while
# the command itself takes one grouped value. Fields without a default stay
# required, which is what the action's contract asserts.
@app.command
def publish(
    request: typ.Annotated[operations.PublishRequest, Parameter(name="*")],
) -> None:
    """Un-draft a fully verified release, or report why it is left unedited.

    Parameters
    ----------
    request
        The tag, revision, repository, and record locations for this run. Each
        field is a flag of its own, so the command line stays ``--tag``,
        ``--commit``, ``--candidate-path`` and so on.

    Notes
    -----
    The ``FinalizeError`` that
    :func:`finalize_github_release_operations.publish_release` raises
    propagates unchanged, with the status it carries: ``1`` when the plan, the
    candidate, or the release refuses publication, ``2`` when the plan still
    requires an upload, ``3`` when an input record cannot be read, ``4`` when
    the outcome cannot be recorded, and ``5`` when a GitHub call fails or
    answers with something unusable. Status ``0`` is not a failure: it is this
    command returning after the release became public with the candidate's
    prerelease state.
    """
    operations.publish_release(request)


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
        standard error. The helpers raise :class:`FinalizeError` instead of
        exiting, so this is the one place the exit vocabulary becomes a
        process status.

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
