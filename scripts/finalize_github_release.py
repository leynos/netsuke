#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# dependencies = ["cyclopts>=4.25.3,<5", "cuprum>=0.1.0,<0.2.0"]
# ///
"""Finalize a verified GitHub release without ever replacing its bytes.

Un-drafting a release is irreversible, so this adapter performs exactly the
two calls that decision needs and nothing more: it reads the release, and it
un-drafts it when the recorded plan says every declared asset already holds
the trusted digest. It never uploads, deletes, or clobbers an asset; a missing
or conflicting asset is a refusal that leaves the release as a draft.

Both properties are enforced below the command line, in
``finalize_github_release_operations``; this module only names the three
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
    candidate after an apparently successful edit.
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
    operations.record_candidate(identity, tag=tag, commit=commit, output=output)


@app.command
def snapshot(
    *,
    tag: typ.Annotated[str, Parameter(required=True)],
    repository: typ.Annotated[str, Parameter(required=True)],
    state_path: typ.Annotated[pathlib.Path, Parameter(required=True)],
    assets_path: typ.Annotated[pathlib.Path, Parameter(required=True)],
) -> None:
    """Record the release's draft flag, prerelease flag, and reported digests."""
    operations.record_snapshot(
        tag, repository, state_path=state_path, assets_path=assets_path
    )


# Cyclopts binds one command-line flag per parameter, so this signature is the
# workflow's interface rather than an incidental long argument list.
@app.command
def publish(  # pylint: disable=too-many-arguments  # one flag per parameter
    *,
    tag: typ.Annotated[str, Parameter(required=True)],
    commit: typ.Annotated[str, Parameter(required=True)],
    repository: typ.Annotated[str, Parameter(required=True)],
    candidate_path: typ.Annotated[pathlib.Path, Parameter(required=True)],
    plan_path: typ.Annotated[pathlib.Path, Parameter(required=True)],
    result_path: typ.Annotated[pathlib.Path, Parameter(required=True)],
    github_output: pathlib.Path | None = None,
) -> None:
    """Un-draft a fully verified release, or report why it stays a draft."""
    operations.publish_release(
        operations.PublishRequest(
            tag=tag,
            commit=commit,
            repository=repository,
            candidate_path=candidate_path,
            plan_path=plan_path,
            result_path=result_path,
            github_output=github_output,
        )
    )


if __name__ == "__main__":
    app()
