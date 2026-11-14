"""Orchestrate the one publishing decision a finalizing run makes.

Un-drafting a release is irreversible, so the guards run before any API call: a
plan that still owes an upload, a plan whose bytes were never established, and a
release that is already public each stop the run before it touches anything.
Only a release in which every declared asset already holds the trusted digest
reaches the one edit this feature performs.

The plan answers for the snapshot the planner read, so the release is
reconciled a second time against the trusted record immediately before the
edit. That second pass is the planner's own policy, not a cheaper copy of it: a
draft that lost or replaced an asset between the two reads is judged exactly as
a first read would judge it. It asks one question the planner does not, because
the planner reconciles only the names the record declares: a draft carrying an
attachment the record never declared is refused, since no digest comparison
could vouch for bytes the record cannot describe. The same reconciliation runs
on the read-back after the edit, and on a retry that finds the release already
public, so a change made in the window between the pre-edit read and the edit is
reported rather than recorded as a success.

Every branch here returns a
:class:`finalize_github_release_decisions.PublishOutcome`. Nothing in this
module raises for a decision, prints, or names an exit status: the command layer
renders the outcome. The two effects it depends on — reading the release and
un-drafting it — are injected ports, defaulting to the Cuprum adapters, so a
test can state what the run decides with no process at all.

The inventory verifier's output is deliberately not an input: comparing GitHub
against a record derived from GitHub would agree with itself however the upload
was tampered with.

Run via ``scripts/finalize_github_release.py``; behavioural coverage lives in
``scripts/tests/test_finalize_github_release_publish.py``,
``scripts/tests/test_finalize_github_release_publish_decisions.py`` and
``scripts/tests/test_finalize_github_release_publish_ports.py``.
"""

import dataclasses

# ``PublishInputs`` reads its field annotations from the class at run time, so
# the types it names are imported here rather than for type checking alone.
import finalize_github_release_decisions as decisions
import finalize_github_release_inventory as release_inventory
import finalize_github_release_io as release_io
import finalize_github_release_plan as release_plan
import finalize_github_release_ports as release_ports  # ruff: ignore[typing-only-third-party-import] - a field annotation names it
import finalize_github_release_remote as release_remote
import finalize_github_release_state as release_state
import release_candidate  # ruff: ignore[typing-only-third-party-import] - a field annotation names it
import release_digest_record  # ruff: ignore[typing-only-third-party-import] - a field annotation names it


@dataclasses.dataclass(frozen=True, slots=True)
class PublishInputs:
    """Name the values one publishing decision is made from.

    Grouping them keeps :func:`publish_release` a one-argument command: the
    decision reads a value, and a test supplies the same value with its own
    ports.

    Attributes
    ----------
    identity
        The validated candidate, which supplies the tag, the revision the
        record was checked against, and the prerelease flag.
    trusted
        The decoded digest record the build declared before upload.
    plan_document
        The decoded reconciliation plan ``finalize_github_release_cli`` wrote.
    repository
        The ``OWNER/REPO`` the release belongs to.
    runner
        The process effect the default ports are built from. It is consulted
        only when ``reader`` or ``editor`` is omitted, so a test that supplies
        both never starts a process.
    reader
        Reads the release. Defaults to the Cuprum adapter.
    editor
        Un-drafts the release. Defaults to the Cuprum adapter.
    """

    identity: release_candidate.ReleaseIdentity
    trusted: release_digest_record.DigestRecord
    plan_document: object
    repository: str
    runner: release_io.Runner = release_io.run_gh
    reader: release_ports.ReleaseReader | None = None
    editor: release_ports.ReleaseEditor | None = None


def publish_release(inputs: PublishInputs) -> decisions.PublishOutcome:
    """Un-draft a fully verified release, or report why it is left unedited.

    Parameters
    ----------
    inputs
        The validated candidate, the trusted record, the plan, and the effects
        the decision asks.

    Returns
    -------
    decisions.PublishOutcome
        :attr:`PublishVerdict.FINALIZED` or
        :attr:`PublishVerdict.ALREADY_FINALIZED` when the release is public,
        and otherwise the first verdict that stopped the run. Every verdict
        other than those two leaves the release unedited, except the ones only
        reachable after the edit was attempted.

    Notes
    -----
    The plan is judged first, from the recorded document alone, so an upload
    still owed or an undecodable plan stops the run before any API call. Only
    then is the release read, and only a draft is reconciled and edited.

    A release already public with the candidate's own flag is reconciled
    against the trusted record and reported as ``already-finalized`` without a
    second edit, so an interrupted retry converges instead of re-publishing.

    The window between the pre-edit read and the edit cannot be closed: GitHub
    offers no conditional edit, so a writer that changes the draft in those
    milliseconds is not observed. What the run can do is notice afterwards. The
    post-edit read-back is reconciled against the same trusted record, and a
    retry that finds the release already public is reconciled too, so a
    mismatch a lost race published is reported by the run that lost it and by
    every pass after it.

    Examples
    --------
    >>> from release_candidate import ReleaseIdentity
    >>> from release_digest_record import DigestRecord
    >>> inputs = PublishInputs(
    ...     identity=ReleaseIdentity("v1.2.3", "0" * 40, "p", "1.2.3", "b", False),
    ...     trusted=DigestRecord.from_mapping({"a.deb": "ab" * 32}),
    ...     plan_document={"actions": [["a.deb", "upload"]]},
    ...     repository="o/r",
    ... )
    >>> publish_release(inputs).verdict.value
    'upload-required'
    """
    plan_outcome = decisions.assess(release_plan.assess_plan(inputs.plan_document))
    if plan_outcome.verdict is not decisions.PublishVerdict.VERIFIED:
        return plan_outcome
    read, edit = _bound_ports(inputs)
    inspected = _inspect_draft(inputs, read)
    if inspected.verdict is not decisions.PublishVerdict.VERIFIED:
        return inspected
    return _edit_and_confirm(inputs, read, edit)


def _bound_ports(
    inputs: PublishInputs,
) -> tuple[release_ports.ReleaseReader, release_ports.ReleaseEditor]:
    """Bind the two effects to the release the candidate names.

    Parameters
    ----------
    inputs
        The candidate, which supplies the tag both effects are bound to, and
        the ports a caller may have supplied instead.

    Returns
    -------
    tuple[release_ports.ReleaseReader, release_ports.ReleaseEditor]
        The read and the edit this run uses, in that order.

    Notes
    -----
    Only a port left unset consults the runner, so a caller that supplied both
    never starts a process.
    """
    read = (
        inputs.reader
        if inputs.reader is not None
        else _reader(inputs.identity.tag, inputs.repository, inputs.runner)
    )
    edit = (
        inputs.editor
        if inputs.editor is not None
        else _editor(inputs.identity.tag, inputs.repository, inputs.runner)
    )
    return read, edit


def _inspect_draft(
    inputs: PublishInputs, read: release_ports.ReleaseReader
) -> decisions.PublishOutcome:
    """Judge the release as the read before any edit found it.

    Parameters
    ----------
    inputs
        The candidate and the trusted record the release is judged against.
    read
        The effect that reads the release.

    Returns
    -------
    decisions.PublishOutcome
        ``VERIFIED`` only for a draft holding the trusted inventory; any other
        verdict stops the run and is reported unchanged.
    """
    current = read(after_edit=False)
    if current.state is None:
        return decisions.remote_failure(current.detail, after_edit=False)
    if release_state.is_already_finalized(
        current.state, is_prerelease=inputs.identity.is_prerelease
    ):
        return _published(inputs.trusted, current.state)
    gate = decisions.draft_gate(current.state)
    if gate.verdict is not decisions.PublishVerdict.VERIFIED:
        return gate
    return release_inventory.draft_inventory(inputs.trusted, current.state)


def _edit_and_confirm(
    inputs: PublishInputs,
    read: release_ports.ReleaseReader,
    edit: release_ports.ReleaseEditor,
) -> decisions.PublishOutcome:
    """Perform the one edit, then judge the release it left behind.

    Parameters
    ----------
    inputs
        The candidate, which supplies the flag the edit carries, and the
        trusted record the read-back is reconciled against.
    read
        The effect that reads the release back.
    edit
        The effect that un-drafts the release.

    Returns
    -------
    decisions.PublishOutcome
        The verdict the read-back supports, or a remote failure when a call
        did not answer.

    Notes
    -----
    A failed edit is not read back: the state is unknown either way. Only an
    accepted edit is confirmed against GitHub.
    """
    answer = edit(is_prerelease=inputs.identity.is_prerelease)
    if answer.exit_code != 0:
        return decisions.remote_failure(answer.detail, after_edit=True)
    published = read(after_edit=True)
    if published.state is None:
        return decisions.remote_failure(published.detail, after_edit=True)
    return release_inventory.readback(
        published.state,
        is_prerelease=inputs.identity.is_prerelease,
        trusted=inputs.trusted,
    )


def _published(
    trusted: release_digest_record.DigestRecord,
    state: release_state.ReleaseState,
) -> decisions.PublishOutcome:
    """Judge the inventory of a release an earlier attempt already published.

    Returns
    -------
    decisions.PublishOutcome
        :attr:`PublishVerdict.ALREADY_FINALIZED` when the inventory still
        matches the trusted record, otherwise
        :attr:`PublishVerdict.PUBLISHED_MISMATCH`.

    Notes
    -----
    The mismatch case must not be recorded as a success: the release is public
    and cannot be made a draft again, so the run reports the disagreement
    rather than leaving it for the next reader to discover.

    Examples
    --------
    >>> from finalize_github_release_state import ReleaseState
    >>> from release_digest_record import DigestRecord
    >>> trusted = DigestRecord.from_mapping({"a.deb": "ab" * 32})
    >>> state = ReleaseState("v1.2.3", False, False, {"a.deb": "sha256:" + "ab" * 32})
    >>> _published(trusted, state).verdict.value
    'already-finalized'
    """
    inventory = release_inventory.published_inventory(trusted, state)
    if inventory.verdict is not decisions.PublishVerdict.VERIFIED:
        return inventory
    return decisions.already_finalized()


def _reader(
    tag: str | None, repository: str, runner: release_io.Runner
) -> release_ports.ReleaseReader:
    """Bind the release read to one tag and repository.

    Parameters
    ----------
    tag
        The release to read. ``require_candidate`` has already refused a
        tagless identity, so the ``None`` branch is defensive only.
    repository
        The ``OWNER/REPO`` the release belongs to.
    runner
        The process effect that answers the read.

    Returns
    -------
    release_ports.ReleaseReader
        A callable taking only the ``after_edit`` fact, so the orchestrator
        names no argument vector.

    Raises
    ------
    ValueError
        If no tag is bound, which no validated candidate can reach.
    """
    if tag is None:
        msg = "a release read needs a tag"
        raise ValueError(msg)

    def read(*, after_edit: bool) -> release_ports.ReleaseRead:
        """Read the bound release."""
        del after_edit  # A read answers the same way whenever it is asked.
        return release_remote.read_release(tag, repository, runner=runner)

    return read


def _editor(
    tag: str | None, repository: str, runner: release_io.Runner
) -> release_ports.ReleaseEditor:
    """Bind the un-drafting edit to one tag and repository.

    Parameters
    ----------
    tag
        The release to un-draft, already validated against the candidate.
    repository
        The ``OWNER/REPO`` the release belongs to.
    runner
        The process effect that performs the edit.

    Returns
    -------
    release_ports.ReleaseEditor
        A callable taking only the prerelease flag, so the flag is the only
        decision the orchestrator has to make about the call.

    Raises
    ------
    ValueError
        If no tag is bound, which no validated candidate can reach.
    """
    if tag is None:
        msg = "a release edit needs a tag"
        raise ValueError(msg)

    def edit(*, is_prerelease: bool) -> release_ports.RemoteAnswer:
        """Un-draft the bound release."""
        return release_remote.edit_release(
            tag, repository, is_prerelease=is_prerelease, runner=runner
        )

    return edit
