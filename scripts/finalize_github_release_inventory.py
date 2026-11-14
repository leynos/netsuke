"""Judge a live release snapshot against the trusted digest record.

The planner answers for the snapshot it read, and its answer travels as a
document. These functions answer the same question about a snapshot this run
took itself: the pre-edit read that guards the one irreversible call, and the
read-back that reports what the edit left behind. Both are compared against the
trusted record the build declared, never against the plan's restatement of it,
because a plan describes a listing that may already be stale.

The pre-edit gate asks two questions together. Reconciliation decides only
about the names the record declares, which leaves an extra attachment
undecided; publishing one would make the release public carrying bytes no
record vouches for, so :func:`draft_inventory` refuses it as well. The read-back
asks only the first question, because it reports a state rather than gating an
edit.

Everything here is a decision over values. Reading a release, running a
command, and naming an exit status belong to the adapters and the command edge.

Run via ``scripts/finalize_github_release.py``; behavioural coverage lives in
``scripts/tests/test_finalize_github_release_publish_decisions.py``.
"""

import typing as typ

import finalize_github_release_decisions as decisions
import finalize_github_release_plan as release_plan
import finalize_github_release_reconcile as release_reconcile
import finalize_github_release_state as release_state

if typ.TYPE_CHECKING:
    # ``DigestRecord`` is named only in annotations, which the Python 3.14
    # baseline resolves lazily, so importing it here buys nothing at run time.
    import release_digest_record

#: What a decision made from this run's own read says it was made from.
_CURRENT_INVENTORY = "the release's current inventory"


def published_inventory(
    trusted: release_digest_record.DigestRecord,
    state: release_state.ReleaseState,
) -> decisions.PublishOutcome:
    """Judge a public release's inventory against the trusted record.

    Parameters
    ----------
    trusted
        The digest record the build declared before upload.
    state
        The public release as a read reported it.

    Returns
    -------
    decisions.PublishOutcome
        :attr:`decisions.PublishVerdict.VERIFIED` when the release still holds
        the trusted inventory, otherwise
        :attr:`decisions.PublishVerdict.PUBLISHED_MISMATCH` naming what
        disagrees.

    Examples
    --------
    >>> from finalize_github_release_state import ReleaseState
    >>> from release_digest_record import DigestRecord
    >>> trusted = DigestRecord.from_mapping({"a.deb": "ab" * 32})
    >>> state = ReleaseState("v1.2.3", False, False, {"a.deb": "sha256:" + "ab" * 32})
    >>> published_inventory(trusted, state).verdict.value
    'verified'
    >>> changed = ReleaseState("v1.2.3", False, False, {"a.deb": "sha256:" + "cd" * 32})
    >>> published_inventory(trusted, changed).conflicts
    ('a.deb',)
    """
    assessment = release_reconcile.assess(trusted, state)
    if assessment.verdict is release_plan.PlanVerdict.VERIFIED:
        return decisions.PublishOutcome(decisions.PublishVerdict.VERIFIED)
    return decisions.PublishOutcome(
        decisions.PublishVerdict.PUBLISHED_MISMATCH,
        uploads=assessment.uploads,
        conflicts=assessment.conflicts,
        unverifiable=assessment.unverifiable,
        detail=assessment.detail,
        is_draft=False,
    )


def readback(
    published: release_state.ReleaseState,
    *,
    is_prerelease: bool,
    trusted: release_digest_record.DigestRecord,
) -> decisions.PublishOutcome:
    """Judge the read-back taken after the un-drafting call.

    Parameters
    ----------
    published
        The release as the post-edit read reported it.
    is_prerelease
        The prerelease state the validated candidate requires.
    trusted
        The digest record the build declared before upload.

    Returns
    -------
    decisions.PublishOutcome
        :attr:`decisions.PublishVerdict.FINALIZED` when the release is public
        with the candidate's flag and the trusted inventory, and the
        mismatching verdict otherwise.

    Examples
    --------
    >>> from finalize_github_release_state import ReleaseState
    >>> from release_digest_record import DigestRecord
    >>> trusted = DigestRecord.from_mapping({"a.deb": "ab" * 32})
    >>> state = ReleaseState("v1.2.3", False, False, {"a.deb": "sha256:" + "ab" * 32})
    >>> readback(state, is_prerelease=False, trusted=trusted).verdict.value
    'finalized'
    >>> readback(state, is_prerelease=True, trusted=trusted).verdict.value
    'prerelease-mismatch'
    """
    match release_state.classify_readback(published, is_prerelease=is_prerelease):
        case release_state.Readback.FINALIZED:
            inventory = published_inventory(trusted, published)
            if inventory.verdict is decisions.PublishVerdict.VERIFIED:
                return decisions.PublishOutcome(decisions.PublishVerdict.FINALIZED)
            return inventory
        case release_state.Readback.STILL_DRAFT:
            return decisions.PublishOutcome(decisions.PublishVerdict.STILL_DRAFT)
        case _:
            return decisions.PublishOutcome(
                decisions.PublishVerdict.PRERELEASE_MISMATCH, is_draft=False
            )


def undeclared_assets(
    trusted: release_digest_record.DigestRecord,
    state: release_state.ReleaseState,
) -> tuple[str, ...]:
    """Return the attached names the trusted record never declared.

    Parameters
    ----------
    trusted
        The digest record the build declared before upload.
    state
        The release as a read reported it.

    Returns
    -------
    tuple[str, ...]
        The attached names absent from the record, in sorted order so the
        diagnosis reads the same however GitHub ordered its listing.

    Notes
    -----
    ``release_reconcile_github.plan_asset_actions`` decides only about the
    names a record declares, so its ``VERIFIED`` verdict says nothing about an
    extra attachment. That is the right split for a plan: reconciliation must
    not invent a verdict about a name it was never given. The finalization gate
    is a different question, asked once, immediately before the irreversible
    edit, so it answers both questions together.

    Examples
    --------
    >>> from finalize_github_release_state import ReleaseState
    >>> from release_digest_record import DigestRecord
    >>> trusted = DigestRecord.from_mapping({"a.deb": "ab" * 32})
    >>> state = ReleaseState(
    ...     "v1.2.3", True, False, {"a.deb": "sha256:" + "ab" * 32, "b.deb": None}
    ... )
    >>> undeclared_assets(trusted, state)
    ('b.deb',)
    """
    declared = trusted.names()
    return tuple(sorted(name for name in state.assets if name not in declared))


def draft_inventory(
    trusted: release_digest_record.DigestRecord,
    state: release_state.ReleaseState,
) -> decisions.PublishOutcome:
    """Judge the draft's inventory immediately before the edit.

    Parameters
    ----------
    trusted
        The digest record the build declared before upload.
    state
        The draft as this run's pre-edit read reported it.

    Returns
    -------
    decisions.PublishOutcome
        :attr:`decisions.PublishVerdict.VERIFIED` when the draft still holds
        exactly the trusted inventory, otherwise the same upload or refusal
        verdict the planner would have reached, with ``is_draft`` true because
        the read saw one. A draft holding the declared inventory plus an
        attachment the record never named is
        :attr:`decisions.PublishVerdict.UNDECLARED_ASSET`.

    Notes
    -----
    The planner tolerates an undeclared extra because it decides only about the
    record's own names. This gate cannot: it is the last read before the one
    irreversible call, and an attachment nobody declared is one the verifier
    would have stopped had it been attached then. Publishing it would make the
    release public carrying bytes no record vouches for, so the run refuses
    instead. A recorded conflict is reported first, because it is the more
    specific diagnosis and names an asset the record does describe.

    Examples
    --------
    >>> from finalize_github_release_state import ReleaseState
    >>> from release_digest_record import DigestRecord
    >>> trusted = DigestRecord.from_mapping({"a.deb": "ab" * 32})
    >>> state = ReleaseState("v1.2.3", True, False, {"a.deb": "sha256:" + "ab" * 32})
    >>> draft_inventory(trusted, state).verdict.value
    'verified'
    >>> extra = ReleaseState(
    ...     "v1.2.3", True, False, {"a.deb": "sha256:" + "ab" * 32, "b.deb": None}
    ... )
    >>> draft_inventory(trusted, extra).undeclared
    ('b.deb',)
    """
    verified = decisions.assess(
        release_reconcile.assess(trusted, state),
        source=_CURRENT_INVENTORY,
        draft_confirmed=True,
    )
    if verified.verdict is not decisions.PublishVerdict.VERIFIED:
        return verified
    undeclared = undeclared_assets(trusted, state)
    if not undeclared:
        return verified
    return decisions.PublishOutcome(
        decisions.PublishVerdict.UNDECLARED_ASSET,
        source=_CURRENT_INVENTORY,
        undeclared=undeclared,
        is_draft=True,
    )
