"""Drive the finalizer's command line and observe the calls it makes.

The adapter's suites need the same way of building an invocation and the same
way of reading back what ``gh`` was asked to do. Keeping that here, beside the
mocked process, means an expected call and the actual one are compared as
tokens rather than as prose.

The records those invocations read live in
``finalize_github_release_records_support``; this module names the command line
and the mocked process, and nothing about a release's contents.
"""

import typing as typ

import finalize_github_release as release_script
from finalize_github_release_records_support import (
    COMMIT,
    REPOSITORY,
    RESULT_NAME,
    TAG,
    PublishRecords,
    identity_record,
    release_view,
    write_candidate,
    write_digest_record,
    write_plan,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import pathlib

    from cmd_mox import CmdMox, CommandDouble, Invocation


def publish_arguments(
    directory: pathlib.Path,
    records: PublishRecords | None = None,
    *,
    github_output: pathlib.Path | None = None,
) -> list[str]:
    """Return a complete ``publish`` invocation over freshly written inputs.

    Parameters
    ----------
    directory
        Directory the three input records and the outcome are written to.
    records
        Overrides for the records a test needs to be malformed. Defaults to a
        well-formed candidate, a verified plan, and the matching digest record.
    github_output
        The workflow output file, when the step asked for outputs.

    Returns
    -------
    list[str]
        Arguments after the program name, ready for :func:`run_finalizer`.

    Notes
    -----
    ``--tag`` is copied from the candidate record, so a test reaches a tag
    disagreement by rewriting that one argument after the helper returns.
    ``--commit`` is always :data:`COMMIT`, so a record written for another
    revision reaches its refusal with no further edit. Between them, the two
    ways a candidate can name the wrong release are both reachable.
    """
    records = PublishRecords() if records is None else records
    record = identity_record() if records.candidate is None else records.candidate
    candidate_path = write_candidate(directory, identity=record)
    plan_path = write_plan(directory, plan=records.plan)
    digest_path = write_digest_record(directory, record=records.trusted)
    arguments = [
        "publish",
        "--tag",
        str(record["tag"]),
        "--commit",
        COMMIT,
        "--repository",
        REPOSITORY,
        "--candidate-path",
        str(candidate_path),
        "--plan-path",
        str(plan_path),
        "--digest-record-path",
        str(digest_path),
        "--result-path",
        str(directory / RESULT_NAME),
    ]
    if github_output is not None:
        arguments.extend(["--github-output", str(github_output)])
    return arguments


def run_finalizer(*arguments: str) -> None:
    """Invoke the adapter as the workflow step would.

    Parameters
    ----------
    *arguments
        The command line after the program name, usually the list
        :func:`publish_arguments` returned.

    Notes
    -----
    The adapter exits through ``SystemExit`` for every refusal, so a test
    asserts on the status it raises rather than on a return value.
    """
    release_script.app(list(arguments), result_action="return_value")


def edit_arguments(tag: str, *, is_prerelease: bool) -> tuple[str, ...]:
    """Return the one edit the finalizer is allowed to make.

    Parameters
    ----------
    tag
        The release to un-draft.
    is_prerelease
        Whether the candidate is a prerelease, which decides the flag the edit
        must carry.

    Returns
    -------
    tuple[str, ...]
        The ``gh release edit`` arguments, pinned so a swapped flag cannot pass
        unnoticed.
    """
    flag = "true" if is_prerelease else "false"
    return (
        "release",
        "edit",
        tag,
        "--repo",
        REPOSITORY,
        "--draft=false",
        f"--prerelease={flag}",
    )


def invocation_arguments(invocation: Invocation) -> tuple[str, ...]:
    """Return an invocation's arguments as a hashable sequence.

    Parameters
    ----------
    invocation
        One call the mocked ``gh`` recorded.

    Returns
    -------
    tuple[str, ...]
        The arguments, comparable with :func:`edit_arguments`' result.
    """
    return tuple(invocation.args)


def sequenced_gh(
    cmd_mox: CmdMox, responses: cabc.Sequence[tuple[str, str, int]]
) -> CommandDouble:
    """Mock ``gh`` to answer with each response in turn.

    Parameters
    ----------
    cmd_mox
        The double-injection fixture the test module requested.
    responses
        ``(stdout, stderr, exit status)`` triples, one per expected call. The
        double is satisfied exactly ``len(responses)`` times, so a run that
        makes an extra call fails rather than reading a stale answer.

    Returns
    -------
    CommandDouble
        The mocked command, whose ``invocations`` record what was called.
    """
    remaining = list(responses)

    def respond(invocation: Invocation) -> tuple[str, str, int]:
        """Answer the next call with the next recorded response."""
        del invocation
        return remaining.pop(0)

    return cmd_mox.mock("gh").runs(respond).times(len(responses))


def release_views(
    *states: tuple[bool, bool], tag: str = TAG
) -> list[tuple[str, str, int]]:
    """Return successful ``gh release view`` responses for each state.

    Parameters
    ----------
    *states
        ``(is_draft, is_prerelease)`` pairs, one per read the run makes.
    tag
        The tag every answer names.

    Returns
    -------
    list[tuple[str, str, int]]
        Responses ready for :func:`sequenced_gh`, in the order given.
    """
    return [
        (
            release_view(is_draft=is_draft, is_prerelease=is_prerelease, tag=tag),
            "",
            0,
        )
        for is_draft, is_prerelease in states
    ]
