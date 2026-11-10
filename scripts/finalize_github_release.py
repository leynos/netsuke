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

Run via ``.github/actions/finalize-github-release``; behavioural coverage
lives in ``scripts/tests/test_finalize_github_release.py``.
"""

# Cyclopts resolves these annotations when it builds the command, so the
# module needs ``pathlib`` at runtime rather than only for type checking.
import pathlib  # ruff: ignore[typing-only-standard-library-import]
import typing as typ

import finalize_github_release_operations as operations
import finalize_github_release_records as records

from cyclopts import App, Parameter

app = App(help="Reconcile and un-draft a verified GitHub release.")


@app.command
def resolve(
    *,
    identity: typ.Annotated[pathlib.Path, Parameter(required=True)],
    tag: typ.Annotated[str, Parameter(required=True)],
    commit: typ.Annotated[str, Parameter(required=True)],
    output: typ.Annotated[pathlib.Path, Parameter(required=True)],
) -> None:
    """Record the candidate the package stage built, after checking its identity."""
    records.record_candidate(identity, tag=tag, commit=commit, output=output)


@app.command
def snapshot(
    *,
    tag: typ.Annotated[str, Parameter(required=True)],
    repository: typ.Annotated[str, Parameter(required=True)],
    state_path: typ.Annotated[pathlib.Path, Parameter(required=True)],
    assets_path: typ.Annotated[pathlib.Path, Parameter(required=True)],
) -> None:
    """Record the release's draft flag, prerelease flag, and reported digests."""
    records.record_snapshot(
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
    """Un-draft a fully verified release, or report why it stays a draft."""
    operations.publish_release(request)


if __name__ == "__main__":
    app()
