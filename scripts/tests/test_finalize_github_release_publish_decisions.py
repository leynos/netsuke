"""State what a publishing run decides, with no process to ask.

A decision is a function of the plan, the trusted record, and what the two
ports answered. This suite supplies those answers directly, so each case is
named for the decision it is about — an upload still owed, a draft that lost an
asset, an edit that may already have landed — without a mocked ``gh`` or the
argument vectors one brings. The adapters' own argv are asserted in
``test_finalize_github_release_publish_ports``; the command line's exit
vocabulary and its prose are asserted in
``test_finalize_github_release_publish``.
"""

import dataclasses
import typing as typ

import finalize_github_release_decisions as decisions
import finalize_github_release_operations as operations
import finalize_github_release_ports as release_ports
from finalize_github_release_records_support import ARCHIVE, DIGEST, REPOSITORY, TAG
from finalize_github_release_state import ReleaseState
from release_candidate import ReleaseIdentity
from release_digest_record import DigestRecord

if typ.TYPE_CHECKING:
    import collections.abc as cabc

COMMIT = "0" * 40
VERIFIED_PLAN = {
    "actions": [[ARCHIVE, "skip-verified"]],
    "conflicts": [],
    "unverifiable": [],
}


@dataclasses.dataclass(slots=True)
class Scripted:
    """Answer the two ports from a script and record what was asked.

    Attributes
    ----------
    reads
        The release reads to answer with, in the order the run asks for them.
    answers
        The edit replies to answer with, in the order the run asks for them.
    read_calls
        Whether each read was asked for after the edit, in call order.
    edit_calls
        The prerelease flag each edit was asked to carry, in call order.
    """

    reads: list[release_ports.ReleaseRead] = dataclasses.field(default_factory=list)
    answers: list[release_ports.RemoteAnswer] = dataclasses.field(default_factory=list)
    read_calls: list[bool] = dataclasses.field(default_factory=list)
    edit_calls: list[bool] = dataclasses.field(default_factory=list)

    def read(self, *, after_edit: bool) -> release_ports.ReleaseRead:
        """Answer one read and record when it was taken."""
        self.read_calls.append(after_edit)
        return self.reads.pop(0)

    def edit(self, *, is_prerelease: bool) -> release_ports.RemoteAnswer:
        """Answer one edit and record the flag it carried."""
        self.edit_calls.append(is_prerelease)
        return self.answers.pop(0)


def draft(
    *, is_prerelease: bool = False, assets: cabc.Mapping[str, str | None] | None = None
) -> release_ports.ReleaseRead:
    """Return a read that established the release is a draft."""
    declared = {ARCHIVE: f"sha256:{DIGEST}"} if assets is None else assets
    return release_ports.ReleaseRead(
        state=ReleaseState(
            tag=TAG,
            is_draft=True,
            is_prerelease=is_prerelease,
            assets=dict(declared),
        )
    )


def public(
    *, is_prerelease: bool = False, assets: cabc.Mapping[str, str | None] | None = None
) -> release_ports.ReleaseRead:
    """Return a read that established the release is public."""
    declared = {ARCHIVE: f"sha256:{DIGEST}"} if assets is None else assets
    return release_ports.ReleaseRead(
        state=ReleaseState(
            tag=TAG,
            is_draft=False,
            is_prerelease=is_prerelease,
            assets=dict(declared),
        )
    )


def candidate(*, is_prerelease: bool = False) -> ReleaseIdentity:
    """Return the validated candidate this run publishes."""
    return ReleaseIdentity(
        TAG, COMMIT, "netsuke-build", "1.2.3", "netsuke", is_prerelease
    )


def trusted() -> DigestRecord:
    """Return the trusted record the build declared before upload."""
    return DigestRecord.from_mapping({ARCHIVE: DIGEST})


def decide(
    script: Scripted,
    plan: object = VERIFIED_PLAN,
    *,
    is_prerelease: bool = False,
) -> decisions.PublishOutcome:
    """Run one publishing decision over the scripted ports."""
    return operations.publish_release(
        operations.PublishInputs(
            identity=candidate(is_prerelease=is_prerelease),
            trusted=trusted(),
            plan_document=plan,
            repository=REPOSITORY,
            reader=script.read,
            editor=script.edit,
        )
    )


def test_a_verified_draft_is_finalized_after_one_edit() -> None:
    """Edit once, then record what the read-back established."""
    script = Scripted(reads=[draft(), public()], answers=[release_ports.RemoteAnswer()])

    outcome = decide(script)

    assert outcome.verdict is decisions.PublishVerdict.FINALIZED, (
        "a draft holding the trusted inventory must be published"
    )
    assert script.read_calls == [False, True], (
        "the run must read before the edit and read back after it"
    )
    assert script.edit_calls == [False], "exactly one edit may be attempted"


def test_the_edit_carries_the_candidate_s_prerelease_flag() -> None:
    """Take the flag from the validated candidate, never from the live release."""
    script = Scripted(
        reads=[draft(is_prerelease=False), public(is_prerelease=True)],
        answers=[release_ports.RemoteAnswer()],
    )

    outcome = decide(script, is_prerelease=True)

    assert script.edit_calls == [True], (
        "the edit must carry the candidate's flag, not the release's"
    )
    assert outcome.verdict is decisions.PublishVerdict.FINALIZED, (
        "an accepted edit must be recorded as publishing the release"
    )


def test_a_plan_that_owes_an_upload_stops_before_the_release_is_read() -> None:
    """Refuse from the recorded plan alone, before any effect is asked."""
    script = Scripted()

    outcome = decide(
        script,
        {
            "actions": [[ARCHIVE, "upload"]],
            "conflicts": [],
            "unverifiable": [],
        },
    )

    assert outcome.verdict is decisions.PublishVerdict.UPLOAD_REQUIRED, (
        "the plan must report the upload it owes"
    )
    assert outcome.uploads == (ARCHIVE,), "the refusal must name the asset owed"
    assert outcome.is_draft is None, (
        "a refusal made from the plan has read nothing, so it cannot claim a state"
    )
    assert not script.read_calls, (
        "an upload still owed must stop the run before GitHub is read"
    )
    assert not script.edit_calls, (
        "an upload still owed must stop the run before GitHub is edited"
    )


def test_a_conflicting_plan_stops_before_the_release_is_read() -> None:
    """Never reconcile live bytes against a plan that already refused."""
    script = Scripted()

    outcome = decide(
        script,
        {
            "actions": [[ARCHIVE, "conflict"]],
            "conflicts": [ARCHIVE],
            "unverifiable": [],
        },
    )

    assert outcome.verdict is decisions.PublishVerdict.REFUSED, (
        "a conflicting plan must be refused"
    )
    assert outcome.conflicts == (ARCHIVE,), "the refusal must name the asset"
    assert not script.read_calls, (
        "a refused plan must stop the run before GitHub is read"
    )
    assert not script.edit_calls, (
        "a refused plan must stop the run before GitHub is edited"
    )


def test_an_unreadable_plan_is_an_input_fault() -> None:
    """Report a plan that cannot support a decision without reading GitHub."""
    script = Scripted()

    outcome = decide(script, {"assets": {}})

    assert outcome.verdict is decisions.PublishVerdict.INPUT_UNREADABLE, (
        "an undecodable plan must be reported as an input fault"
    )
    assert outcome.detail, "an input fault must carry the decoding diagnosis"
    assert not script.read_calls, "no release is read for an unusable plan"


def test_an_already_public_release_is_not_edited_again() -> None:
    """Converge on a retry that finds an earlier attempt already published."""
    script = Scripted(reads=[public()])

    outcome = decide(script)

    assert outcome.verdict is decisions.PublishVerdict.ALREADY_FINALIZED, (
        "a public release must be recorded as already finalized"
    )
    assert outcome.is_draft is False, "the read established the release is public"
    assert not script.edit_calls, "an already-public release must not be edited"


def test_a_public_release_with_the_wrong_flag_is_not_a_draft() -> None:
    """Refuse a public release the candidate would have to re-classify."""
    script = Scripted(reads=[public(is_prerelease=True)])

    outcome = decide(script)

    assert outcome.verdict is decisions.PublishVerdict.NOT_A_DRAFT, (
        "a public release with the other flag must be refused"
    )
    assert outcome.is_draft is False, "the read established the release is public"
    assert not script.edit_calls, "a public release cannot be un-drafted"


def test_a_retry_that_finds_changed_bytes_reports_a_published_mismatch() -> None:
    """Report a lost race rather than record it as a success."""
    script = Scripted(reads=[public(assets={ARCHIVE: "sha256:" + "cd" * 32})])

    outcome = decide(script)

    assert outcome.verdict is decisions.PublishVerdict.PUBLISHED_MISMATCH, (
        "a public release that lost the trusted bytes must be reported"
    )
    assert outcome.conflicts == (ARCHIVE,), "the mismatch must name the asset"
    assert not script.edit_calls, "a mismatched release must not be edited"


def test_an_unreadable_pre_edit_read_leaves_the_state_unknown() -> None:
    """Say nothing about a release the read did not establish."""
    script = Scripted(
        reads=[release_ports.ReleaseRead(detail="gh release view exited 5")]
    )

    outcome = decide(script)

    assert outcome.verdict is decisions.PublishVerdict.REMOTE_FAILED, (
        "a failed read must be reported as a remote failure"
    )
    assert outcome.is_draft is None, "a failed read establishes no draft state"
    assert outcome.after_edit is False, "no edit was attempted"
    assert not script.edit_calls, "an unreadable release must not be edited"


def test_a_draft_that_lost_a_declared_asset_owes_an_upload() -> None:
    """Judge the pre-edit read by the planner's own policy."""
    script = Scripted(reads=[draft(assets={})])

    outcome = decide(script)

    assert outcome.verdict is decisions.PublishVerdict.UPLOAD_REQUIRED, (
        "a draft missing a declared asset must owe its upload"
    )
    assert outcome.uploads == (ARCHIVE,), "the lost asset must be named"
    assert outcome.is_draft is True, "the read established the release is a draft"
    assert not script.edit_calls, "a draft missing an asset must not be published"


def test_a_draft_whose_bytes_changed_is_refused() -> None:
    """Stop before the edit when the draft disagrees with the trusted record.

    The listing also holds an undeclared attachment, which proves the conflict
    is reported first: it names an asset the record does describe.
    """
    assets = {ARCHIVE: "sha256:" + "cd" * 32, "notes.txt": f"sha256:{DIGEST}"}
    script = Scripted(reads=[draft(assets=assets)])

    outcome = decide(script)

    assert outcome.verdict is decisions.PublishVerdict.REFUSED, (
        "a changed declared asset must be refused before an undeclared one is named"
    )
    assert outcome.conflicts == (ARCHIVE,), "the refusal must name the asset"
    assert outcome.is_draft is True, "the read established the release is a draft"
    assert not script.edit_calls, "a changed asset must not be published"


def test_a_draft_holding_an_undeclared_attachment_is_refused() -> None:
    """Refuse the last read before the edit when the record cannot describe it.

    The planner decides only about the names the record declares, so its
    verdict leaves an extra attachment undecided. Nothing later would check it:
    the release would become public carrying bytes no record vouches for.
    """
    script = Scripted(
        reads=[draft(assets={ARCHIVE: f"sha256:{DIGEST}", "notes.txt": None})]
    )

    outcome = decide(script)

    assert outcome.verdict is decisions.PublishVerdict.UNDECLARED_ASSET, (
        "an attachment the record never declared must stop the run"
    )
    assert outcome.undeclared == ("notes.txt",), (
        "the refusal must name the attachment the record cannot describe"
    )
    assert outcome.is_draft is True, "the read established the release is a draft"
    assert not script.edit_calls, "an undeclared attachment must not be published"


def test_a_failed_edit_is_reported_as_possibly_applied() -> None:
    """Report the state as unknown once an edit may have landed."""
    script = Scripted(
        reads=[draft()],
        answers=[
            release_ports.RemoteAnswer(exit_code=1, detail="gh release edit exited 1")
        ],
    )

    outcome = decide(script)

    assert outcome.verdict is decisions.PublishVerdict.REMOTE_FAILED, (
        "a failed edit must be reported as a remote failure"
    )
    assert outcome.after_edit is True, "an attempted edit may already have landed"
    assert outcome.is_draft is None, "an attempted edit cannot be called a draft"
    assert script.read_calls == [False], "a failed edit must not be read back"


def test_a_readback_that_contradicts_the_candidate_is_a_mismatch() -> None:
    """Report a public release the read-back found with the wrong flag."""
    script = Scripted(
        reads=[draft(), public(is_prerelease=True)],
        answers=[release_ports.RemoteAnswer()],
    )

    outcome = decide(script)

    assert outcome.verdict is decisions.PublishVerdict.PRERELEASE_MISMATCH, (
        "a read-back with the other flag must be reported"
    )
    assert outcome.is_draft is False, "the read-back established the release is public"


def test_a_readback_that_is_still_a_draft_is_reported_as_such() -> None:
    """Never record a success for a release ``gh`` left a draft."""
    script = Scripted(reads=[draft(), draft()], answers=[release_ports.RemoteAnswer()])

    outcome = decide(script)

    assert outcome.verdict is decisions.PublishVerdict.STILL_DRAFT, (
        "an accepted edit that left a draft must not be recorded as a success"
    )
    assert outcome.is_draft is None, "the outcome does not claim the edit failed"


def test_a_readback_that_cannot_be_read_leaves_the_state_unknown() -> None:
    """Report a lost read-back after an edit as possibly published."""
    script = Scripted(
        reads=[draft(), release_ports.ReleaseRead(detail="gh release view exited 1")],
        answers=[release_ports.RemoteAnswer()],
    )

    outcome = decide(script)

    assert outcome.verdict is decisions.PublishVerdict.REMOTE_FAILED, (
        "a lost read-back must be reported as a remote failure"
    )
    assert outcome.after_edit is True, "the edit was attempted before the read-back"
    assert outcome.is_draft is None, "a lost read-back establishes no state"
