"""Decide and record what a registry exchange should do, then verify its bytes.

The registry round-trip itself belongs to the workflow step: it holds the
credential, and this module deliberately holds none. What it does is the part
that must be *decided* rather than performed — read the answer the registry
gave, classify it against the digest this run would send, and state whether the
run should publish, stand down, or stop. :mod:`release_reconcile_crates` owns
that classification and is proved separately; this CLI is the seam that lets a
workflow step call it.

The verification half is the same seam in reverse. After a publication, the
step downloads the archive it just published and hands the bytes here, where
:func:`verify_crates_io_bytes.verify_downloaded_archive` decides whether they
are the archive the identity record describes.

Neither half opens a socket. A step that does is a step, and says so in the
workflow where its credential lives.
"""

import argparse
import json
import pathlib
import sys

import release_digest
import release_reconcile_crates
import verify_crates_io_bytes

EXIT_OK = 0
EXIT_ACTION_REQUIRED = 1
EXIT_VERIFICATION_FAILED = 2
EXIT_USAGE = 64

#: The verdicts :func:`verify_crates_io_bytes.plan_verification` can return,
#: paired with the exit code this CLI reports for each. Enumerated as a table
#: so a verdict added to the policy module without a branch here fails the
#: consistency test rather than falling through to a default.
VERDICT_EXITS = {
    "verified": EXIT_OK,
    "absent": EXIT_VERIFICATION_FAILED,
    "mismatch": EXIT_VERIFICATION_FAILED,
    "error": EXIT_VERIFICATION_FAILED,
}


def build_parser() -> argparse.ArgumentParser:
    """Build the parser for the registry-exchange CLI.

    Returns
    -------
    argparse.ArgumentParser
        A parser with two mutually exclusive modes: ``classify`` reads a
        recorded registry response, and ``verify`` checks downloaded bytes.
    """
    parser = argparse.ArgumentParser(
        prog="publish_crates_io_cli",
        description="Classify a registry response or verify a published archive.",
    )
    mode = parser.add_subparsers(dest="command", required=True)

    classify = mode.add_parser(
        "classify",
        help="decide whether to publish, skip, or fail from a recorded response",
    )
    classify.add_argument(
        "--status", required=True, type=int, help="HTTP status the registry answered"
    )
    classify.add_argument(
        "--body",
        type=pathlib.Path,
        help="file holding the response body, omitted when the response had none",
    )
    classify.add_argument(
        "--sha256", required=True, help="digest this run would publish"
    )
    classify.add_argument(
        "--output", required=True, type=pathlib.Path, help="where to write the decision"
    )

    verify = mode.add_parser(
        "verify", help="check a downloaded archive against the recorded digest"
    )
    verify.add_argument(
        "--archive", required=True, type=pathlib.Path, help="the downloaded .crate"
    )
    verify.add_argument("--crate", required=True, help="the crate name")
    verify.add_argument("--version", required=True, help="the published version")
    verify.add_argument(
        "--sha256", required=True, help="the digest the identity record holds"
    )
    verify.add_argument(
        "--output", required=True, type=pathlib.Path, help="where to write the verdict"
    )
    return parser


def _classify(arguments: argparse.Namespace) -> int:
    """Classify a registry response and record the action it dictates."""
    body = (
        arguments.body.read_text(encoding="utf-8")
        if arguments.body is not None
        else None
    )
    state = release_reconcile_crates.classify_registry_state(
        body, intended_sha256=arguments.sha256, http_status=arguments.status
    )
    action = release_reconcile_crates.plan_crates_action(state)
    arguments.output.write_text(
        json.dumps({"action": action, "state": state.value}, indent=2, sort_keys=True)
        + "\n",
        encoding="utf-8",
    )
    if action == "fail":
        print(
            f"registry state {state.value!r} does not permit publication",
            file=sys.stderr,
        )
        return EXIT_ACTION_REQUIRED
    return EXIT_OK


def _verify(arguments: argparse.Namespace) -> int:
    """Verify a downloaded archive, recording the digest that was read.

    The exit code comes from :data:`VERDICT_EXITS` rather than from a branch
    per exception, so the table is the single place that says which verdicts
    stop a release. A verdict the policy module can reach but this table does
    not name is caught by the consistency test instead of by a silent default.

    Returns
    -------
    int
        ``EXIT_OK`` when the downloaded bytes matched the expected digest, or
        ``EXIT_VERIFICATION_FAILED`` when the archive was absent or disagreed.
    """
    try:
        digest = verify_crates_io_bytes.verify_downloaded_archive(
            arguments.archive,
            crate=arguments.crate,
            version=arguments.version,
            expected_sha256=arguments.sha256,
        )
    except verify_crates_io_bytes.ArchiveAbsentError as error:
        print(f"published archive absent: {error}", file=sys.stderr)
        return VERDICT_EXITS["absent"]
    except release_digest.DigestMismatchError as error:
        print(f"published archive mismatch: {error}", file=sys.stderr)
        return VERDICT_EXITS["mismatch"]
    arguments.output.write_text(
        json.dumps({"sha256": digest, "verdict": "verified"}, indent=2, sort_keys=True)
        + "\n",
        encoding="utf-8",
    )
    return VERDICT_EXITS["verified"]


def main(argv: list[str] | None = None) -> int:
    """Run the selected mode and return its exit code.

    Parameters
    ----------
    argv
        Command-line arguments. Defaults to ``sys.argv[1:]``.

    Returns
    -------
    int
        ``EXIT_OK`` when the decision permits proceeding and the bytes verify,
        or a non-zero code naming the refusal.

    Notes
    -----
    The classification is written even when it refuses, so an operator reading
    the job's artefacts sees the state that caused the stop rather than only
    an exit code. A verification failure writes nothing: there is no verdict
    to record beyond the message on standard error.

    Examples
    --------
    A registry answer of ``404`` means the version is not there, so the action
    is to publish. ``/dev/null`` stands in for the destination, because a
    doctest may not leave a file behind:

    >>> main(["classify", "--status", "404", "--sha256", "ab",
    ...       "--output", "/dev/null"])
    0
    """
    arguments = build_parser().parse_args(argv)
    match arguments.command:
        case "classify":
            return _classify(arguments)
        case "verify":
            return _verify(arguments)
        case other:
            print(f"unknown command {other!r}", file=sys.stderr)
            return EXIT_USAGE


if __name__ == "__main__":
    raise SystemExit(main())
