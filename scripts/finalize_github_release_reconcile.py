"""Reconcile a live release snapshot against the trusted digest record.

The planner reconciles the snapshot it read. This module applies that same
policy to a snapshot taken later, by the same run. Two later reads need it: the
pre-edit read, which guards the one mutating call, and the post-edit read-back,
which is the only place a writer that changed the draft between the two can be
observed. Reusing the planner's own reconciliation rather than a cheaper
comparison keeps every caller judging an asset exactly as the plan judged it,
including the rule that an undeclared extra attachment is tolerated.

A record that cannot be read is an input fault, never a verified release: an
absent record would reconcile nothing, and no disagreement is the shape of a
release the record verified.

Run via ``scripts/finalize_github_release.py``; behavioural coverage lives in
``scripts/tests/test_finalize_github_release_publish.py``.
"""

import typing as typ

import finalize_github_release_io as release_io
import finalize_github_release_plan as release_plan
import release_digest_record
import release_digest_record_files
import release_github_assets_adapter
import release_reconcile_github
import release_upload_messages

if typ.TYPE_CHECKING:
    # Every name here appears only in a signature: the record path is passed
    # through untouched, the state is read by the planner, and the identity is
    # read by nothing.
    import pathlib

    import finalize_github_release_state as release_state
    import release_candidate


def reconcile(
    digest_record_path: pathlib.Path, state: release_state.ReleaseState
) -> release_plan.PlanAssessment:
    """Re-run the planner's reconciliation against the release as it is now.

    Parameters
    ----------
    digest_record_path
        The trusted digest record the declaration stage wrote before upload.
    state
        The release as a snapshot taken by this run reported it.

    Returns
    -------
    release_plan.PlanAssessment
        The verdict the planner would have reached for this listing.

    Raises
    ------
    FinalizeError
        With :data:`EXIT_INPUT_UNREADABLE` when the record cannot be read.

    Notes
    -----
    The record is re-read rather than taken from the plan, so the comparison is
    against the bytes the build declared and not against a document the planner
    wrote. The digest conversion is
    :func:`release_github_assets_adapter.normalise_github_digest`, the same one
    the planner uses, so an unusable digest is unverifiable here for the same
    reason it was there.
    """
    try:
        expected = release_digest_record_files.read_digest_record(digest_record_path)
    except release_digest_record.DigestRecordError as error:
        raise release_io.FinalizeError(
            release_io.EXIT_INPUT_UNREADABLE,
            f"{digest_record_path} is not a usable digest record: {error}",
        ) from error
    present = {
        name: release_github_assets_adapter.normalise_github_digest(digest)
        for name, digest in state.assets.items()
    }
    return release_plan.assess_actions(
        release_reconcile_github.plan_asset_actions(
            expected=expected.as_mapping(), present=present
        )
    )


def inventory_mismatch(
    digest_record_path: pathlib.Path, state: release_state.ReleaseState
) -> str | None:
    """Return why a release's inventory disagrees with the trusted record.

    Returns
    -------
    str | None
        The asset names that disagree, or ``None`` when the record verifies
        against the release. An assessment that threatens no upload but names
        no disagreement is described by how many declared assets are missing,
        so a verdict with no conflicts still produces a diagnosis.

    Notes
    -----
    The ``FinalizeError`` that :func:`reconcile` raises propagates unchanged,
    with :data:`EXIT_INPUT_UNREADABLE` when the record cannot be read.

    Examples
    --------
    >>> import json
    >>> import pathlib
    >>> import tempfile
    >>> from finalize_github_release_state import ReleaseState
    >>> with tempfile.TemporaryDirectory() as directory:
    ...     record = pathlib.Path(directory) / "digests.json"
    ...     _ = record.write_text(
    ...         json.dumps({"assets": {"a.deb": "ab" * 32}}), encoding="utf-8"
    ...     )
    ...     trusted = ReleaseState(
    ...         "v1.2.3", True, False, {"a.deb": "sha256:" + "ab" * 32}
    ...     )
    ...     print(inventory_mismatch(record, trusted))
    ...     changed = ReleaseState(
    ...         "v1.2.3", True, False, {"a.deb": "sha256:" + "cd" * 32}
    ...     )
    ...     print(inventory_mismatch(record, changed))
    None
    the release holds assets that disagree with the trusted record: a.deb
    """
    result = reconcile(digest_record_path, state)
    if result.verdict is release_plan.PlanVerdict.VERIFIED:
        return None
    return (
        release_upload_messages.blocked_message(result.conflicts, result.unverifiable)
        or f"{len(result.uploads)} declared asset(s) are no longer attached"
    )


def refuse_published_mismatch(
    digest_record_path: pathlib.Path,
    identity: release_candidate.ReleaseIdentity,
    state: release_state.ReleaseState,
) -> None:
    """Stop when a public release no longer holds the trusted inventory.

    Parameters
    ----------
    digest_record_path
        The trusted digest record the build declared before upload.
    identity
        The validated candidate, named in the diagnosis.
    state
        The public release as the most recent read reported it.

    Raises
    ------
    FinalizeError
        With :data:`EXIT_REFUSED` when the release is public and its inventory
        disagrees with the record. GitHub cannot make a public release a draft
        again, so the run cannot undo the edit; it names what disagrees
        instead of recording a success nobody can rely on.

    Notes
    -----
    The pre-edit read and the edit are two calls, and GitHub offers no
    conditional edit, so a writer that changes the draft between them is not
    observed. Reconciling the read-back against the record is what turns that
    race into a reported mismatch. The same check runs on a retry that finds
    the release already public, so a mismatch a lost race published is reported
    by the next pass rather than standing until an operator happens to find it.
    """
    mismatch = inventory_mismatch(digest_record_path, state)
    if mismatch is None:
        return
    raise release_io.FinalizeError(
        release_io.EXIT_REFUSED,
        f"release {identity.tag} is public, but {mismatch}; GitHub cannot make "
        "a public release a draft again, so this run cannot repair it: replace "
        "the named asset, or withdraw the release, before announcing it",
    )
