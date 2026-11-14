"""Prove the finalizer's two GitHub adapters ask and decode correctly.

The adapters own the only two calls a finalizing run makes, so this suite pins
each argument vector token by token and each answer's meaning: a refusal is a
value the decision reports, a payload for another release is unusable, and the
read-only snapshot makes one ``gh release view`` and no edit. The process is
the injected runner, so nothing here starts ``gh``.

What the run decides from those answers is asserted in
``test_finalize_github_release_publish_decisions``; what the workflow reads back
is asserted in ``test_finalize_github_release_publish``. The composition itself
is asserted here too, where the argv are already visible: a run built from one
runner must ask exactly one edit, and none at all when it refuses.
"""

import dataclasses
import json

import cuprum
import finalize_github_release_decisions as decisions
import finalize_github_release_io as release_io
import finalize_github_release_operations as operations
import finalize_github_release_ports as release_ports
import finalize_github_release_records as release_records
import finalize_github_release_remote as release_remote
import pytest
from finalize_github_release_records_support import (
    ARCHIVE,
    DIGEST,
    REPOSITORY,
    TAG,
    VIEW_ARGUMENTS,
    plan_record,
    release_view,
)
from finalize_github_release_support import edit_arguments
from release_candidate import ReleaseIdentity
from release_digest_record import DigestRecord

#: The revision the composed run publishes, fixed so the identity is the one a
#: healthy run accepts rather than one derived from the answer under test.
COMMIT = "0" * 40


def _result(
    *,
    exit_code: int = 0,
    stdout: str | None = "",
    stderr: str | None = "",
) -> cuprum.CommandResult:
    """Return the result Cuprum reports for one finished ``gh`` call."""
    return cuprum.CommandResult(
        program=cuprum.Program("gh"),
        argv=("gh",),
        exit_code=exit_code,
        pid=0,
        stdout=stdout,
        stderr=stderr,
    )


@dataclasses.dataclass(slots=True)
class Answers:
    """Answer every process an adapter starts, and record what it asked.

    Attributes
    ----------
    result
        The single answer every call receives.
    calls
        One ``(arguments, keywords)`` pair per call, in call order.
    """

    result: cuprum.CommandResult
    calls: list[tuple[tuple[object, ...], dict[str, object]]] = dataclasses.field(
        default_factory=list
    )

    def __call__(self, *args: object, **kwargs: object) -> cuprum.CommandResult:
        """Record one call and answer it."""
        self.calls.append((args, kwargs))
        return self.result


def test_read_release_asks_for_the_recorded_fields() -> None:
    """Read the four fields the decoder requires, and decode them."""
    runner = Answers(_result(stdout=release_view(is_draft=True, is_prerelease=False)))

    read = release_remote.read_release(TAG, REPOSITORY, runner=runner)

    assert runner.calls == [(VIEW_ARGUMENTS, {})], (
        "the read must name the tag, repository, and JSON fields once"
    )
    assert read.state is not None, "a well-formed answer must decode"
    assert read.state.is_draft is True, "the draft flag must be decoded"
    assert read.state.assets == {ARCHIVE: f"sha256:{DIGEST}"}, (
        "each attached asset's digest must be decoded"
    )


def test_read_release_reports_a_failed_view_as_a_detail() -> None:
    """Carry a refusal as a value, naming the command and its diagnostic."""
    runner = Answers(_result(exit_code=5, stdout=None, stderr="HTTP 403\n"))

    read = release_remote.read_release(TAG, REPOSITORY, runner=runner)

    assert read.state is None, "a failed command establishes no state"
    assert read.detail == "gh release view exited 5: HTTP 403", (
        "the diagnosis must name the command and keep gh's own words"
    )


def test_read_release_drops_a_diagnostic_that_is_only_whitespace() -> None:
    """Leave no dangling colon when the command wrote only a line break."""
    runner = Answers(_result(exit_code=5, stdout=None, stderr="  \n"))

    read = release_remote.read_release(TAG, REPOSITORY, runner=runner)

    assert read.detail == "gh release view exited 5", (
        "a stderr with nothing in it must add no clause"
    )


def test_read_release_reports_an_unusable_response() -> None:
    """Refuse output that is not JSON rather than guess at a state."""
    runner = Answers(_result(stdout="not json"))

    read = release_remote.read_release(TAG, REPOSITORY, runner=runner)

    assert read.state is None, "malformed output establishes no state"
    assert "unusable response" in read.detail, (
        "malformed output must be reported as unusable, not as a draft"
    )


def test_read_release_refuses_a_payload_for_another_tag() -> None:
    """Never read another release's state as this one's."""
    runner = Answers(
        _result(stdout=release_view(is_draft=True, is_prerelease=False, tag="v9.9.9"))
    )

    read = release_remote.read_release(TAG, REPOSITORY, runner=runner)

    assert read.state is None, "a response for another tag must not decode"
    assert "v9.9.9" in read.detail, "the diagnosis must name what was reported"


def test_edit_release_builds_the_edit_and_flags_itself_for_the_log() -> None:
    """Un-draft with the candidate's flag and echo the call into the log."""
    runner = Answers(_result())

    answer = release_remote.edit_release(
        TAG, REPOSITORY, is_prerelease=True, runner=runner
    )

    assert runner.calls == [
        (edit_arguments(TAG, is_prerelease=True), {"echo": True})
    ], "the edit must carry the candidate's flag and be echoed to the workflow"
    assert answer == release_ports.RemoteAnswer(), "an accepted edit is an empty answer"


def test_edit_release_reports_a_refused_edit() -> None:
    """Carry a refused edit as an answer rather than an exception."""
    runner = Answers(_result(exit_code=1, stdout=None, stderr="boom\n"))

    answer = release_remote.edit_release(
        TAG, REPOSITORY, is_prerelease=False, runner=runner
    )

    assert answer.exit_code == 1, "a refused edit must carry its exit status"
    assert answer.detail == "gh release edit exited 1: boom", (
        "the diagnosis must keep gh's own words"
    )


def test_snapshot_release_makes_one_view_and_no_edit() -> None:
    """Record the state from one read, without any other call."""
    runner = Answers(_result(stdout=release_view(is_draft=True, is_prerelease=True)))

    state = release_records.snapshot_release(TAG, REPOSITORY, runner=runner)

    assert state.is_draft is True, "the snapshot must record the draft flag"
    assert state.is_prerelease is True, "the snapshot must record the prerelease flag"
    assert len(runner.calls) == 1, "the snapshot stage may read the release once"
    assert runner.calls[0][0][:2] == ("release", "view"), (
        "the snapshot stage must never edit the release"
    )


def test_snapshot_release_fails_when_the_view_fails() -> None:
    """Stop the read-only stage with a diagnosis that claims no draft state."""
    runner = Answers(_result(exit_code=5, stdout=None, stderr="HTTP 403\n"))

    with pytest.raises(release_io.FinalizeError) as error:
        release_records.snapshot_release(TAG, REPOSITORY, runner=runner)

    assert error.value.code == release_io.EXIT_REMOTE_FAILED, (
        "a failed read reports the remote-failure status"
    )
    assert "state is unknown" in str(error.value), (
        "a failed read must not promise the release is a draft"
    )


@dataclasses.dataclass(slots=True)
class Replies:
    """Answer each command in turn, so a run may ask more than one.

    ``Answers`` gives every call the same answer, which is right for a single
    adapter. A composed run asks a read, then an edit, then a read-back, and
    needs a different answer to each, so this variant consumes one result per
    call and records the argv it was asked for.

    Attributes
    ----------
    results
        The answers left to give, in call order.
    calls
        The positional arguments of each call, in call order.
    """

    results: list[cuprum.CommandResult] = dataclasses.field(default_factory=list)
    calls: list[tuple[object, ...]] = dataclasses.field(default_factory=list)

    def __call__(self, *args: object, **kwargs: object) -> cuprum.CommandResult:
        """Record one call and answer it from the script."""
        del kwargs  # Only the arguments decide what the command was.
        self.calls.append(args)
        return self.results.pop(0)


def _composed(runner: Replies) -> operations.PublishInputs:
    """Build the inputs the command layer builds, from one runner.

    Returns
    -------
    operations.PublishInputs
        The decoded candidate, the trusted record, a verified plan, and the two
        ports bound to the release the candidate names.
    """
    return operations.PublishInputs(
        identity=ReleaseIdentity(
            TAG, COMMIT, "netsuke-build", "1.2.3", "netsuke", is_prerelease=False
        ),
        trusted=DigestRecord.from_mapping({ARCHIVE: DIGEST}),
        plan_document=plan_record(),
        reader=release_remote.reader_port(TAG, REPOSITORY, runner=runner),
        editor=release_remote.editor_port(TAG, REPOSITORY, runner=runner),
    )


def test_a_composed_run_edits_the_release_exactly_once() -> None:
    """Bind both ports from one runner and edit between the two reads."""
    runner = Replies(
        results=[
            _result(stdout=release_view(is_draft=True, is_prerelease=False)),
            _result(),
            _result(stdout=release_view(is_draft=False, is_prerelease=False)),
        ]
    )

    outcome = operations.publish_release(_composed(runner))

    assert outcome.verdict is decisions.PublishVerdict.FINALIZED, (
        "a verified draft whose edit GitHub accepted must be recorded as published"
    )
    assert [call[:2] for call in runner.calls] == [
        ("release", "view"),
        ("release", "edit"),
        ("release", "view"),
    ], "the run must read, edit once, and read back, in that order"


def test_a_composed_refusal_edits_nothing() -> None:
    """Stop a draft that lost a declared asset after its one read."""
    runner = Replies(
        results=[
            _result(stdout=release_view(is_draft=True, is_prerelease=False, assets={}))
        ]
    )

    outcome = operations.publish_release(_composed(runner))

    assert outcome.verdict is decisions.PublishVerdict.UPLOAD_REQUIRED, (
        "a draft missing a declared asset must owe its upload"
    )
    assert [call[:2] for call in runner.calls] == [("release", "view")], (
        "a refusal before the edit must ask no further command"
    )


def test_a_payload_naming_a_repeated_asset_is_unusable() -> None:
    """Refuse an ambiguous asset list rather than decide which entry wins."""
    payload = json.dumps({
        "tagName": TAG,
        "isDraft": True,
        "isPrerelease": False,
        "assets": [{"name": ARCHIVE}, {"name": ARCHIVE}],
    })
    runner = Answers(_result(stdout=payload))

    read = release_remote.read_release(TAG, REPOSITORY, runner=runner)

    assert read.state is None, "an ambiguous payload establishes no state"
    assert "more than once" in read.detail, "the diagnosis must name the repeated asset"
