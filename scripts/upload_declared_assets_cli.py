"""Upload exactly the declared release assets, never clobbering what exists.

The upload step is the only part of publication that writes to GitHub, and it
is a re-run as often as a first run: an interrupted job restarts, a tag is
pushed twice, an operator retries a failed transfer. ``gh release upload
--clobber`` makes that "work" by replacing whatever is attached, which would
let a second run built from another commit publish its bytes under the first
run's tag. ``--clobber`` is never passed here.

Nor is "no clobber" enough on its own: without reconciliation the second
upload simply fails, and the operator learns nothing about *which* asset
blocked it or whether the bytes already attached were the right ones. So the
step compares digests first, using the trusted record the selection stage wrote
before any byte moved, and only the names that record declares:

- an attached asset whose digest matches is skipped, silently and safely;
- an asset the release has not attached is uploaded;
- an attached asset whose digest was read and differs is a conflict;
- an attached asset whose digest GitHub did not report and a download could
  not establish is *unverifiable* -- a different finding from a conflict, with
  a different remedy, because nothing disagrees and the fault may be in the
  listing or the transfer rather than in the release;
- *every* conflict and unverifiable name is reported before the first upload,
  so a run never publishes a partial inventory over a wrong one;
- an attached asset the record does not declare is ignored here. It is not
  this step's to reconcile, and treating it as a conflict would block a
  release over a file the plan never claimed.

Listing the release and downloading an asset it hid are effectful and live in
:mod:`release_upload_observation`; deciding what to do about what was observed
is the pure query beside them. Separating the two lets the decision be stated
as a table of observations and tested without a stub, and leaves exactly one
function that reaches the outside world. This module is the argument surface
and the exit-status contract over that boundary.

Two records arrive with the payload, and the step requires them to agree before
it acts. Neither proves the copy on its own: a staged tree replaced between
the two hashes yields a declaration record that faithfully describes the
replacement. Only the two records answering for one another closes that
window, and it happens before the first network call.

The digests are checked twice: once against the payload on disk before any
network call, and again from the release listing after the upload. The second
pass is what proves the transfer delivered the bytes that were selected rather
than merely that ``gh`` exited zero.

A dry run lists and plans but uploads nothing, so the plan is still meaningful
for a caller rehearsing a release.

The plan is a decision over names and digests; reading the release and
sending the transfer live in :mod:`release_upload_remote`, so that module is
the only one a test has to stub to keep the network out of the suite.

Run via ``.github/actions/upload-declared-assets``; behavioural coverage lives
in ``scripts/tests/test_upload_declared_assets_cli.py``.
"""

import argparse
import pathlib
import sys

import release_digest_record
import release_digest_record_files
import release_payload_tree
import release_reconcile_github
import release_upload_observation
import release_upload_plan
import release_upload_remote
import upload_declared_assets_refusals
from release_upload_plan import PLAN_NAME, UploadRequest
from release_upload_remote import PRODUCTION_RUNNER, SubprocessRunner
from upload_declared_assets_refusals import (
    EXIT_CONFLICT,
    EXIT_INPUT_UNREADABLE,
    EXIT_OK,
    EXIT_OUTPUT_UNWRITABLE,
    EXIT_UPLOAD_FAILED,
)

__all__ = [
    "EXIT_CONFLICT",
    "EXIT_INPUT_UNREADABLE",
    "EXIT_OK",
    "EXIT_OUTPUT_UNWRITABLE",
    "EXIT_UPLOAD_FAILED",
    "PLAN_NAME",
    "UploadRequest",
    "build_parser",
    "main",
    "settle",
]


def build_parser() -> argparse.ArgumentParser:
    """Build the parser for the declared-asset uploader.

    Returns
    -------
    argparse.ArgumentParser
        A parser requiring the release tag, the selected payload, and the
        trusted record, and optionally planning without uploading.

    Examples
    --------
    >>> arguments = build_parser().parse_args([
    ...     "--release-tag", "v1.0.0",
    ...     "--payload-dir", "artifacts/payload/payload",
    ...     "--digest-record", "artifacts/record/trusted.json",
    ...     "--selection-record", "artifacts/record/selection.json",
    ...     "--plan", "artifacts/record/upload-plan.json",
    ... ])
    >>> arguments.dry_run
    False
    """
    parser = argparse.ArgumentParser(
        prog="upload_declared_assets_cli",
        description="Upload exactly the declared assets a release is missing.",
    )
    parser.add_argument("--release-tag", required=True)
    parser.add_argument("--payload-dir", required=True, type=pathlib.Path)
    parser.add_argument(
        "--digest-record",
        required=True,
        type=pathlib.Path,
        help="the declaration record written from the staged tree",
    )
    parser.add_argument(
        "--selection-record",
        required=True,
        type=pathlib.Path,
        help="the record selection wrote from the staged sources it verified",
    )
    parser.add_argument("--plan", required=True, type=pathlib.Path)
    parser.add_argument("--dry-run", action="store_true")
    return parser


def main(argv: list[str] | None = None) -> int:
    """Reconcile the release against the trusted record, uploading only gaps.

    Parameters
    ----------
    argv
        Command-line arguments. Defaults to ``sys.argv[1:]``.

    Returns
    -------
    int
        ``EXIT_OK`` when every declared asset is attached and verified,
        ``EXIT_CONFLICT`` when the payload or the release disagrees with the
        trusted record, ``EXIT_INPUT_UNREADABLE`` when the record or the
        payload cannot be read, ``EXIT_UPLOAD_FAILED`` when ``gh`` cannot
        complete a required call or the release still lacks a declared asset
        after the transfer, or ``EXIT_OUTPUT_UNWRITABLE`` when the plan
        cannot be recorded.

    Examples
    --------
    An absent trusted record is refused before ``gh`` is invoked at all:

    >>> main(["--release-tag", "v1.0.0", "--payload-dir", "/nonexistent",
    ...       "--digest-record", "/nonexistent/record.json",
    ...       "--selection-record", "/nonexistent/selection.json",
    ...       "--plan", "/dev/null"])
    2
    """
    arguments = build_parser().parse_args(argv)
    request = UploadRequest(
        tag=arguments.release_tag,
        payload_dir=arguments.payload_dir,
        digest_record=arguments.digest_record,
        selection_record=arguments.selection_record,
        plan_path=arguments.plan,
        is_dry_run=arguments.dry_run,
    )
    try:
        actions = settle(request, runner=PRODUCTION_RUNNER)
    except upload_declared_assets_refusals.REFUSALS as error:
        exit_code, label = upload_declared_assets_refusals.refusal(error)
        print(f"{label}: {error}", file=sys.stderr)
        return exit_code
    conflicts = release_reconcile_github.conflicts(actions)
    unverifiable = release_reconcile_github.unverifiable(actions)
    try:
        release_upload_plan.write_plan(
            request.plan_path, actions, conflicts, unverifiable
        )
    except OSError as error:
        destination = request.plan_path
        print(f"upload plan write failed: {destination}: {error}", file=sys.stderr)
        return EXIT_OUTPUT_UNWRITABLE
    if conflicts or unverifiable:
        print(
            release_upload_plan.blocked_message(conflicts, unverifiable),
            file=sys.stderr,
        )
        return EXIT_CONFLICT
    # A plan that still asks for an upload means GitHub does not hold that
    # asset: `gh release upload` can succeed while the asset never appears, and
    # `plan_asset_actions` reports an absent name as UPLOAD rather than
    # CONFLICT. The docstring above promises this exit means every declared
    # asset is attached and verified, so an absent one is a failed upload, not
    # a clean run. A dry run legitimately ends in UPLOAD and is left alone.
    unverified = () if request.is_dry_run else release_reconcile_github.uploads(actions)
    if unverified:
        missing = ", ".join(unverified)
        print(f"declared assets not attached after upload: {missing}", file=sys.stderr)
        return EXIT_UPLOAD_FAILED
    return EXIT_OK


def settle(
    request: UploadRequest, *, runner: SubprocessRunner
) -> tuple[tuple[str, release_reconcile_github.AssetAction], ...]:
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
    tuple[tuple[str, release_reconcile_github.AssetAction], ...]
        The final per-asset plan, sorted by name.

    Notes
    -----
    :func:`verified_payload` runs before the first network call. The release
    is re-listed after the upload, so the returned plan describes what GitHub
    actually holds rather than what was sent. Every conflict is reported
    before any upload, so a run with one conflicting asset publishes none of
    the others.

    Examples
    --------
    A dry run plans the upload but sends nothing:

    >>> import tempfile
    >>> from pathlib import Path
    >>> import release_digest_record_files, release_digest_files
    >>> import release_upload_remote
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
    ...     actions = settle(
    ...         UploadRequest("v1", payload, record, selection,
    ...                       root / "plan.json", is_dry_run=True),
    ...         runner=release_upload_remote.StaticRunner('{"assets": []}'),
    ...     )
    >>> [action.value for _name, action in actions]
    ['upload']
    """
    expected = verified_payload(request)
    actions = release_upload_observation.reconcile(request.tag, expected, runner=runner)
    pending = release_reconcile_github.uploads(actions)
    if release_upload_observation.should_upload(request, actions, pending):
        release_upload_remote.upload(
            request.tag, pending, request.payload_dir, runner=runner
        )
        return release_upload_observation.reconcile(
            request.tag, expected, runner=runner
        )
    return actions


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
    readers and the comparison, and ``release_payload_tree.PayloadHandoffError``
    from :func:`release_payload_tree.require_payload_matches`.

    Examples
    --------
    >>> import tempfile
    >>> from pathlib import Path
    >>> import release_digest_record_files, release_digest_files
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
    release_digest_record.require_records_agree(selected, declaration=declaration)
    release_payload_tree.require_payload_matches(request.payload_dir, declaration)
    return declaration


if __name__ == "__main__":
    raise SystemExit(main())
