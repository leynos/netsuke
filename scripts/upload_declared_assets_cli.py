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
- an attached asset whose digest differs -- or whose digest GitHub cannot
  report and a download cannot establish -- is a conflict, and *every* conflict
  is reported before the first upload, so a run never publishes a partial
  inventory over a wrong one;
- an attached asset the record does not declare is ignored here. It is not
  this step's to reconcile, and treating it as a conflict would block a
  release over a file the plan never claimed.

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
import typing as typ

import release_digest_record_files
import release_payload_tree
import release_reconcile_github
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

if typ.TYPE_CHECKING:
    import collections.abc as cabc

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
        help="the trusted record selection wrote from the staged bytes",
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
    ...       "--plan", "/dev/null"])
    2
    """
    arguments = build_parser().parse_args(argv)
    request = UploadRequest(
        tag=arguments.release_tag,
        payload_dir=arguments.payload_dir,
        digest_record=arguments.digest_record,
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
    try:
        release_upload_plan.write_plan(request.plan_path, actions, conflicts)
    except OSError as error:
        destination = request.plan_path
        print(f"upload plan write failed: {destination}: {error}", file=sys.stderr)
        return EXIT_OUTPUT_UNWRITABLE
    if conflicts:
        print(release_upload_plan.conflict_message(conflicts), file=sys.stderr)
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
    The payload is verified against the trusted record before the first
    network call, and the release is re-listed after the upload so the
    returned plan describes what GitHub actually holds rather than what was
    sent. Every conflict is reported before any upload, so a run with one
    conflicting asset publishes none of the others.

    Examples
    --------
    A dry run plans the upload but sends nothing, and grants no digest it did
    not itself compute:

    >>> import tempfile
    >>> from pathlib import Path
    >>> import release_digest_files
    >>> import release_digest_record_files
    >>> import release_upload_remote
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     root = Path(tmp)
    ...     payload = root / "payload"
    ...     payload.mkdir()
    ...     asset = payload / "a.deb"
    ...     _ = asset.write_bytes(b"deb")
    ...     record = root / "record.json"
    ...     release_digest_record_files.write_digest_record(
    ...         {"a.deb": release_digest_files.digest_of_file(asset)}, record
    ...     )
    ...     request = UploadRequest(
    ...         tag="v1", payload_dir=payload, digest_record=record,
    ...         plan_path=root / "plan.json", is_dry_run=True,
    ...     )
    ...     actions = settle(
    ...         request,
    ...         runner=release_upload_remote.StaticRunner('{"assets": []}'),
    ...     )
    >>> [action.value for _name, action in actions]
    ['upload']
    """
    expected = _expected(request.digest_record)
    release_payload_tree.require_payload_matches(request.payload_dir, expected)
    actions = _reconcile(request.tag, expected, runner=runner)
    pending = release_reconcile_github.uploads(actions)
    if _should_upload(request, actions, pending):
        release_upload_remote.upload(
            request.tag, pending, request.payload_dir, runner=runner
        )
        return _reconcile(request.tag, expected, runner=runner)
    return actions


def _should_upload(
    request: UploadRequest,
    actions: cabc.Iterable[tuple[str, release_reconcile_github.AssetAction]],
    pending: cabc.Iterable[str],
) -> bool:
    """Report whether this run has a transfer to make.

    Returns
    -------
    bool
        ``False`` for a dry run, a plan with nothing to upload, or a plan that
        already contains a conflict.

    Notes
    -----
    A conflict blocks every upload even when other names are missing. Sending
    the missing ones first would leave the release holding a mixture of this
    run's bytes and an earlier run's, which is harder to unwind than a refusal.

    Examples
    --------
    >>> from release_reconcile_github import AssetAction
    >>> request = UploadRequest("v1", pathlib.Path(), pathlib.Path(),
    ...                         pathlib.Path(), is_dry_run=True)
    >>> _should_upload(request, [("a", AssetAction.UPLOAD)], ("a",))
    False
    >>> _should_upload(UploadRequest("v1", pathlib.Path(), pathlib.Path(),
    ...                              pathlib.Path(), is_dry_run=False),
    ...                [("a", AssetAction.CONFLICT), ("b", AssetAction.UPLOAD)],
    ...                ("b",))
    False
    """
    return (
        not request.is_dry_run
        and bool(tuple(pending))
        and not tuple(release_reconcile_github.conflicts(actions))
    )


def _expected(digest_record: pathlib.Path) -> dict[str, str]:
    """Read the trusted record that names every declared asset and its digest.

    Returns
    -------
    dict[str, str]
        Published name mapped to the digest selection recorded for it.

    Notes
    -----
    ``release_digest_record.DigestRecordError`` and its
    ``RecordedDigestMismatchError`` subclass reach the caller from the record
    reader; nothing here raises one.
    """
    return release_digest_record_files.read_digest_record(digest_record)


def _reconcile(
    tag: str, expected: cabc.Mapping[str, str], *, runner: SubprocessRunner
) -> tuple[tuple[str, release_reconcile_github.AssetAction], ...]:
    """Plan the release's assets from a listing, backfilling absent digests.

    Parameters
    ----------
    tag
        The validated release tag to list.
    expected
        Published name mapped to the digest the trusted record demands.
    runner
        The injected subprocess seam.

    Returns
    -------
    tuple[tuple[str, release_reconcile_github.AssetAction], ...]
        One ``(name, action)`` pair per declared asset, sorted by name.

    Notes
    -----
    A listing that cannot be taken surfaces ``GhCommandFailedError`` or
    ``RemoteListingError`` from :mod:`release_upload_remote`.
    """
    present = release_upload_remote.attached_digests(tag, runner=runner)
    undigested = {name for name, digest in present.items() if digest is None}
    if undigested:
        present |= release_upload_remote.downloaded_digests(
            tag, undigested, runner=runner
        )
    return release_reconcile_github.plan_asset_actions(
        expected=expected, present=present
    )


if __name__ == "__main__":
    raise SystemExit(main())
