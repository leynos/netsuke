"""Reconcile a live release snapshot against the trusted digest record.

The planner reconciles the snapshot it read. This module applies that same
policy to a snapshot taken later, by the same run. Two later reads need it: the
pre-edit read, which guards the one mutating call, and the post-edit read-back,
which is the only place a writer that changed the draft between the two can be
observed. Reusing the planner's own reconciliation rather than a cheaper
comparison keeps every caller judging an asset exactly as the plan judged it,
including the rule that an undeclared extra attachment is tolerated.

The record arrives decoded. Reading it is an adapter's job
(:func:`finalize_github_release_io.load_digest_record`), so this module holds no
path and performs no I/O: a caller that already holds the trusted record can
ask what a snapshot permits without touching the filesystem. A record that
could not be read never reaches here, because an absent record would reconcile
nothing and no disagreement is the shape of a release the record verified.

Run via ``scripts/finalize_github_release.py``; behavioural coverage lives in
``scripts/tests/test_finalize_github_release_publish.py``.
"""

import typing as typ

import finalize_github_release_plan as release_plan
import release_github_assets_adapter
import release_reconcile_github

if typ.TYPE_CHECKING:
    # Both names appear only in annotations. The state is read by the planner
    # and this module never constructs one; the record arrives already decoded.
    import finalize_github_release_state as release_state
    import release_digest_record


def assess(
    trusted: release_digest_record.DigestRecord,
    state: release_state.ReleaseState,
) -> release_plan.PlanAssessment:
    """Re-run the planner's reconciliation against the release as it is now.

    Parameters
    ----------
    trusted
        The trusted digest record the declaration stage wrote before upload,
        already decoded and validated.
    state
        The release as a snapshot taken by this run reported it.

    Returns
    -------
    release_plan.PlanAssessment
        The verdict the planner would have reached for this listing.

    Notes
    -----
    The digest conversion is
    :func:`release_github_assets_adapter.normalise_github_digest`, the same one
    the planner uses, so an unusable digest is unverifiable here for the same
    reason it was there.

    Examples
    --------
    >>> from release_digest_record import DigestRecord
    >>> from finalize_github_release_state import ReleaseState
    >>> trusted = DigestRecord.from_mapping({"a.deb": "ab" * 32})
    >>> current = ReleaseState("v1.2.3", True, False, {"a.deb": "sha256:" + "ab" * 32})
    >>> assess(trusted, current).verdict.value
    'verified'
    >>> changed = ReleaseState("v1.2.3", True, False, {"a.deb": "sha256:" + "cd" * 32})
    >>> assess(trusted, changed).conflicts
    ('a.deb',)
    """
    present = {
        name: release_github_assets_adapter.normalise_github_digest(digest)
        for name, digest in state.assets.items()
    }
    return release_plan.assess_actions(
        release_reconcile_github.plan_asset_actions(
            expected=trusted.as_mapping(), present=present
        )
    )
