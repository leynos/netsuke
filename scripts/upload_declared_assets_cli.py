"""Upload exactly the declared release assets, never clobbering what exists.

Publication writes to GitHub in three places, and this is the only one that
attaches release *assets*. ``ensure_draft_release.py`` creates the release
itself as a draft, one asset upload sends the payload's bytes, and the
finalizer un-drafts the release once the inventory is verified. Creating a
draft is a different operation from filling it: a draft is invisible until it
is published, so a re-run can find it already there, whereas a second asset
upload lands on a release an operator can already see. That is why the
no-clobber rule and the reconciliation below belong to this step and not to
the two around it.

It is a re-run as often as a first run: an interrupted job restarts, a tag is
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
and the exit-status contract over that boundary; the run itself — verify the
payload, transfer, re-observe — is :func:`release_upload_run.settle`.

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
import os
import pathlib
import sys
import typing as typ

import release_reconcile_github
import release_upload_messages
import release_upload_plan
import release_upload_timing_files
import release_upload_trace
import upload_declared_assets_refusals
from release_upload_plan import PLAN_NAME, UploadRequest
from release_upload_remote import PRODUCTION_RUNNER
from release_upload_run import settle
from release_upload_timing import TimedRunner
from upload_declared_assets_refusals import (
    EXIT_CONFLICT,
    EXIT_INPUT_UNREADABLE,
    EXIT_OK,
    EXIT_OUTPUT_UNWRITABLE,
    EXIT_UPLOAD_FAILED,
)

if typ.TYPE_CHECKING:
    import release_upload_observation

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
    # Every boundary a run crosses is a `gh` invocation, so timing the one
    # subprocess seam measures the listing, the download, and the transfer
    # without any call site knowing this wrapper exists. The same seam traces
    # each call, which is what separates "the transfer was slow" from "the
    # listing retried and the transfer never began".
    timed = TimedRunner(
        PRODUCTION_RUNNER, context=release_upload_trace.trace_context(os.environ)
    )
    try:
        settlement = settle(request, runner=timed)
    except upload_declared_assets_refusals.REFUSALS as error:
        exit_code, label = upload_declared_assets_refusals.refusal(error)
        print(f"{label}: {error}", file=sys.stderr)
        _record_measurements(request, timed)
        return exit_code
    # After the plan, so the records describe one run: a timings file but no
    # plan is a run that measured boundaries it never finished reporting.
    status = _report(request, settlement)
    _record_measurements(request, timed)
    return status


def _record_measurements(request: UploadRequest, timed: TimedRunner[object]) -> None:
    """Write the aggregate timings and the per-call trace beside the plan.

    Parameters
    ----------
    request
        The run whose plan destination names the directory.
    timed
        The seam that measured and traced every boundary the run crossed.

    Notes
    -----
    The trace goes second so a run that dies writing it has still published the
    aggregate, and neither file can fail the run: the plan is the contract, and
    the uploader has already said on standard error why it refused.
    """
    release_upload_timing_files.record_timings(request.plan_path, timed.timings)
    release_upload_trace.record_trace(request.plan_path, timed.trace)


def _report(
    request: UploadRequest, settlement: release_upload_observation.Settlement
) -> int:
    """Record the plan, report every blocker, and settle the exit status.

    Parameters
    ----------
    request
        The run that produced ``settlement``, for its plan destination and its
        dry-run flag.
    settlement
        The per-asset plan, and the bounded reasons behind any asset whose
        digest could not be established.

    Returns
    -------
    int
        ``EXIT_CONFLICT`` when the plan holds a blocker, ``EXIT_OK`` when every
        declared asset is attached and verified, otherwise the status for the
        failure that stopped the run.

    Notes
    -----
    The plan is written before any status is decided, so a refused run still
    leaves an operator the record of what it planned. Blockers are derived from
    the plan rather than assembled beside it, so the file and the message cannot
    disagree about which asset blocked the run.

    A plan that still asks for an upload means GitHub does not hold that asset:
    ``gh release upload`` can succeed while the asset never appears, and
    ``plan_asset_actions`` reports an absent name as ``UPLOAD`` rather than
    ``CONFLICT``. :func:`main` promises this exit means every declared asset is
    attached and verified, so an absent one is a failed upload, not a clean run.
    A dry run legitimately ends in ``UPLOAD`` and is left alone.
    """
    actions = settlement.actions
    blockers = release_upload_plan.Blockers.of(actions, settlement.failures)
    try:
        release_upload_plan.write_plan(request.plan_path, actions, blockers)
    except OSError as error:
        destination = request.plan_path
        print(f"upload plan write failed: {destination}: {error}", file=sys.stderr)
        return EXIT_OUTPUT_UNWRITABLE
    if blockers.conflicts or blockers.unverifiable:
        print(
            release_upload_messages.blocked_message(
                blockers.conflicts, blockers.unverifiable, blockers.reasons
            ),
            file=sys.stderr,
        )
        return EXIT_CONFLICT
    unverified = () if request.is_dry_run else release_reconcile_github.uploads(actions)
    if unverified:
        missing = ", ".join(unverified)
        print(f"declared assets not attached after upload: {missing}", file=sys.stderr)
        return EXIT_UPLOAD_FAILED
    return EXIT_OK


if __name__ == "__main__":
    raise SystemExit(main())
