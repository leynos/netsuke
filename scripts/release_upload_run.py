"""Run one declared-asset upload: verify the payload, transfer, re-observe.

An upload run is three things in a fixed order, and the order is the contract.
The payload is first judged against the two records that arrived with it, so a
tree that lost, gained, or replaced a file is refused before a single byte
moves. Only then is the release listed and the plan decided, and only a plan
that asks for a transfer is sent. Finally the release is listed again, because
what ``gh release upload`` reports is that a command exited, not that GitHub
holds the bytes.

The sequence lives here rather than in
:mod:`upload_declared_assets_cli` so that module can stay what its own
docstring says it is: the argument surface and the exit-status contract over
this boundary. Nothing here reads ``sys.argv``, prints a diagnostic, or chooses
an exit status; a refusal leaves by exception and the CLI is what names it.

Every effect is reached through an injected seam.
:func:`release_upload_observation.observe_release` and
:func:`release_upload_remote.upload` take the runner the caller supplied, so a
test keeps the network out of the suite without patching a module global, and
:func:`verified_payload` reads records and files through the adapters that own
those formats.

Run via ``scripts/upload_declared_assets_cli.py``; behavioural coverage lives in
``scripts/tests/test_upload_declared_assets_cli.py`` and
``scripts/tests/test_upload_declared_assets_status.py``.
"""

import typing as typ

import release_digest_record_agreement
import release_digest_record_files
import release_payload_handoff
import release_reconcile_github
import release_upload_observation
import release_upload_plan
import release_upload_remote

if typ.TYPE_CHECKING:
    from release_upload_plan import UploadRequest
    from release_upload_remote import SubprocessRunner

__all__ = ["settle", "verified_payload"]


def settle(
    request: UploadRequest, *, runner: SubprocessRunner
) -> release_upload_observation.Settlement:
    """Decide, upload, and re-verify one release's declared assets.

    Parameters
    ----------
    request
        The tag, payload, trusted record, plan destination, and dry-run flag.
    runner
        The subprocess seam. Production supplies ``subprocess.run``; a test
        supplies a stub, so no test reaches GitHub.

    Returns
    -------
    release_upload_observation.Settlement
        The final plan, and the bounded reasons behind any unverified asset.

    Notes
    -----
    :func:`verified_payload` runs before the first network call, and every
    conflict is reported before any upload, so one conflicting asset publishes
    none of the others. Observing again after the transfer is what makes the
    returned plan describe what GitHub holds now.

    Examples
    --------
    A dry run plans the upload but sends nothing:

    >>> import tempfile
    >>> from pathlib import Path
    >>> import release_digest_record_files, release_digest_files
    >>> import release_upload_remote
    >>> from release_upload_plan import UploadRequest
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     root = Path(tmp)
    ...     payload = root / "payload"
    ...     payload.mkdir()
    ...     asset = payload / "a.deb"
    ...     _ = asset.write_bytes(b"deb")
    ...     digests = {"a.deb": release_digest_files.digest_of_file(asset)}
    ...     record, selection = root / "r.json", root / "s.json"
    ...     for destination in (record, selection):
    ...         release_digest_record_files.write_digest_record(
    ...             digests, destination)
    ...     settlement = settle(
    ...         UploadRequest("v1", payload, record, selection,
    ...                       root / "plan.json", is_dry_run=True),
    ...         runner=release_upload_remote.StaticRunner('{"assets": []}'),
    ...     )
    >>> [action.value for _name, action in settlement.actions]
    ['upload']
    """
    expected = verified_payload(request)
    settlement = _observed_settlement(request.tag, expected, runner=runner)
    pending = release_reconcile_github.uploads(settlement.actions)
    if not release_upload_observation.should_upload(
        request, settlement.actions, pending
    ):
        return settlement
    # Record the decided plan before the first byte moves. A transfer that
    # raises leaves by exception, so a plan written only afterwards would be
    # absent from exactly the run an operator most needs it for: the one that
    # failed partway. Writing here costs a second, smaller write when the
    # transfer succeeds, which is the price of the plan surviving a failure.
    _record_plan(request, settlement)
    release_upload_remote.upload(
        request.tag, pending, request.payload_dir, runner=runner
    )
    # A second observation of the same release, not a second decision
    # procedure: the returned plan has to describe what GitHub holds now.
    return _observed_settlement(request.tag, expected, runner=runner)


def _record_plan(
    request: UploadRequest, settlement: release_upload_observation.Settlement
) -> None:
    """Write the plan this run has decided, before it is acted on.

    Raises
    ------
    release_upload_plan.UnwritablePlanError
        If the destination cannot be written. Raised rather than swallowed: a
        run that cannot record what it is about to do must not go on to do it,
        and the refusal table gives this its own status so the diagnosis does
        not read as an unreadable payload.
    """
    blockers = release_upload_plan.Blockers.of(settlement.actions, settlement.failures)
    try:
        release_upload_plan.write_plan(request.plan_path, settlement.actions, blockers)
    except OSError as error:
        raise release_upload_plan.UnwritablePlanError(str(error)) from error


def _observed_settlement(
    tag: str, expected: dict[str, str], *, runner: SubprocessRunner
) -> release_upload_observation.Settlement:
    """List the release once and turn that observation into a plan.

    Returns
    -------
    release_upload_observation.Settlement
        The plan for the release as GitHub holds it at this moment, and the
        reasons behind any digest that could not be established.

    Notes
    -----
    Listing and deciding stay two calls with one value between them:
    :func:`release_upload_observation.observe_release` is the only half that
    touches the network or the filesystem, and ``settle_observation`` reads
    what it returns and nothing else. Composing them here keeps that boundary
    while letting :func:`settle` name the two moments it observes -- once to
    decide, once to confirm after a transfer -- without restating the pair.

    Examples
    --------
    An empty listing leaves the one declared asset to upload:

    >>> import release_upload_remote
    >>> settlement = _observed_settlement(
    ...     "v1", {"a.deb": "0" * 64},
    ...     runner=release_upload_remote.StaticRunner('{"assets": []}'),
    ... )
    >>> [(name, action.value) for name, action in settlement.actions]
    [('a.deb', 'upload')]
    """
    return release_upload_observation.settle_observation(
        release_upload_observation.observe_release(tag, expected, runner=runner),
        expected=expected,
    )


def verified_payload(request: UploadRequest) -> dict[str, str]:
    """Return the digests the payload must hold, refusing any other bytes.

    Parameters
    ----------
    request
        The payload directory and the two records that describe it.

    Returns
    -------
    dict[str, str]
        Published name mapped to the digest both records agree on.

    Notes
    -----
    The declaration record is written from the staged tree after selection has
    hashed the sources it approved, so a tree replaced in between yields a
    declaration record that faithfully describes the replacement. The
    selection record is the independent account; only once the two name the
    same bytes is the payload compared with either.

    ``release_digest_record.DigestRecordError`` and its
    ``RecordedDigestMismatchError`` subclass reach the caller from the record
    readers and the comparison, and
    ``release_payload_handoff.PayloadHandoffError`` from
    :func:`release_payload_handoff.require_payload_matches`.

    Examples
    --------
    >>> import tempfile
    >>> from pathlib import Path
    >>> import release_digest_record_files, release_digest_files
    >>> from release_upload_plan import UploadRequest
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     root = Path(tmp)
    ...     (root / "payload").mkdir()
    ...     asset = root / "payload" / "a.deb"
    ...     _ = asset.write_bytes(b"deb")
    ...     digests = {"a.deb": release_digest_files.digest_of_file(asset)}
    ...     record, selection = root / "r.json", root / "s.json"
    ...     for destination in (record, selection):
    ...         release_digest_record_files.write_digest_record(
    ...             digests, destination)
    ...     verified_payload(
    ...         UploadRequest("v1", root / "payload", record, selection,
    ...                       root / "plan.json", is_dry_run=False)
    ...     ) == digests
    True
    """
    declaration = release_digest_record_files.read_digest_record(request.digest_record)
    selected = release_digest_record_files.read_digest_record(request.selection_record)
    declared = declaration.as_mapping()
    release_digest_record_agreement.require_records_agree(
        selected.as_mapping(), declaration=declared
    )
    release_payload_handoff.require_payload_matches(request.payload_dir, declared)
    return declared
