"""Plan and verify the public-consumer smoke for one release candidate.

``plan`` binds the trusted evidence to the exact public downloads a smoke run
must exercise: the crate on ``static.crates.io`` and the cargo-binstall archive
with its checksum sidecar on the GitHub release. ``verify`` re-derives that plan
from the same evidence, refuses a saved plan that disagrees, and then checks the
bytes the workflow downloaded.

Nothing here fetches anything. The workflow performs the downloads with a
credential-free ``curl`` and this CLI reads what was saved, so the smoke
exercises the same public URLs a consumer uses while the Python side stays
network-free and testable.

Decisions live in :mod:`release_consumer_smoke` and
:mod:`release_consumer_payload`, the trusted-record reads in
:mod:`consumer_smoke_evidence`, and the phase walk in
:mod:`consumer_smoke_verify`, which also owns the exit codes each refusal
reports:

- ``0`` every phase verified.
- ``1`` the identity, trusted record, declared archive set, or saved plan is
  unusable, or the verdict artefact cannot be written.
- ``2`` a public download is missing, empty, or not the bytes we published.
- ``3`` the archive itself is refused: corrupt, unsafe to extract, or
  unwritable.
- ``4`` the shipped executable could not report the candidate's exact version:
  it could not be launched, did not answer in time, exited non-zero, or printed
  something else.

Run from ``.github/actions/consumer-smoke/action.yml``; behavioural coverage
lives in ``scripts/tests/test_consumer_smoke_cli.py``.
"""

import argparse
import pathlib
import sys
import typing as typ

import consumer_smoke_evidence as evidence
import consumer_smoke_verify as verify
import release_consumer_smoke as smoke
from consumer_smoke_verify import EXIT_EVIDENCE_REJECTED, EXIT_OK

if typ.TYPE_CHECKING:
    import release_binstall_probe as probe_adapter

#: The default release staging configuration, relative to the repository root.
DEFAULT_STAGING_CONFIG = pathlib.Path(".github/release-staging.toml")


def run_plan(arguments: argparse.Namespace) -> int:
    """Write the plan document for this run's evidence.

    Returns
    -------
    int
        :data:`consumer_smoke_verify.EXIT_OK` once the plan is written, or the
        evidence-rejected status when the evidence is unreadable or unusable.
    """
    try:
        plan = smoke.plan_smoke(evidence.gather_evidence(arguments))
        evidence.write_json(arguments.output, evidence.plan_document(plan))
    except (smoke.EvidenceRejectedError, OSError) as error:
        print(f"consumer smoke evidence rejected: {error}", file=sys.stderr)
        return EXIT_EVIDENCE_REJECTED
    return EXIT_OK


def _evidence_parser() -> argparse.ArgumentParser:
    """Build the parser holding the evidence both commands share."""
    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument(
        "--identity",
        required=True,
        type=pathlib.Path,
        help="the package record the packaging stage wrote",
    )
    parser.add_argument(
        "--trusted",
        required=True,
        type=pathlib.Path,
        help="the pre-upload digest record, keyed by published asset name",
    )
    parser.add_argument(
        "--staging-config",
        type=pathlib.Path,
        default=DEFAULT_STAGING_CONFIG,
        help="the release staging configuration naming every target",
    )
    parser.add_argument(
        "--manifest",
        type=pathlib.Path,
        default=pathlib.Path("Cargo.toml"),
        help="the Cargo manifest naming the package",
    )
    parser.add_argument(
        "--repository", required=True, help="the 'owner/name' GitHub slug"
    )
    parser.add_argument("--target", required=True, help="the Rust target triple")
    parser.add_argument(
        "--commit", required=True, help="the commit this run checked out"
    )
    return parser


def build_parser() -> argparse.ArgumentParser:
    """Build the parser for the consumer-smoke CLI.

    Returns
    -------
    argparse.ArgumentParser
        A parser whose ``plan`` and ``verify`` subcommands share the evidence
        arguments.
    """
    parser = argparse.ArgumentParser(
        prog="consumer_smoke_cli",
        description="Plan and verify the public-consumer release smoke.",
    )
    commands = parser.add_subparsers(dest="command", required=True)
    plan = commands.add_parser(
        "plan",
        parents=[_evidence_parser()],
        help="derive the public downloads this run must perform",
    )
    plan.add_argument(
        "--output",
        required=True,
        type=pathlib.Path,
        help="where to write the plan document",
    )
    verify_command = commands.add_parser(
        "verify",
        parents=[_evidence_parser()],
        help="check the saved downloads and the shipped executable",
    )
    verify_command.add_argument(
        "--plan",
        required=True,
        type=pathlib.Path,
        help="the plan document this run re-derives and enforces",
    )
    verify_command.add_argument(
        "--crate", required=True, type=pathlib.Path, help="the saved crate archive"
    )
    verify_command.add_argument(
        "--crate-status",
        required=True,
        type=pathlib.Path,
        help="the HTTP status curl recorded for the crate",
    )
    verify_command.add_argument(
        "--archive", required=True, type=pathlib.Path, help="the saved binstall archive"
    )
    verify_command.add_argument(
        "--archive-status",
        required=True,
        type=pathlib.Path,
        help="the HTTP status curl recorded for the archive",
    )
    verify_command.add_argument(
        "--sidecar",
        required=True,
        type=pathlib.Path,
        help="the saved checksum sidecar",
    )
    verify_command.add_argument(
        "--sidecar-status",
        required=True,
        type=pathlib.Path,
        help="the HTTP status curl recorded for the sidecar",
    )
    verify_command.add_argument(
        "--extract-dir",
        required=True,
        type=pathlib.Path,
        help="a fresh, empty directory the binary is extracted into",
    )
    verify_command.add_argument(
        "--verdict",
        required=True,
        type=pathlib.Path,
        help="where to write the bounded phase-and-verdict record",
    )
    verify_command.add_argument(
        "--phase-log",
        required=True,
        type=pathlib.Path,
        help="where to write the bounded per-phase outcome and timing record",
    )
    return parser


def main(
    argv: list[str] | None = None,
    *,
    runner: probe_adapter.SubprocessRunner | None = None,
) -> int:
    """Run the requested command and report its outcome.

    Parameters
    ----------
    argv
        Command-line arguments. Defaults to ``sys.argv[1:]``.
    runner
        The callable that launches the extracted binary. ``None`` uses
        :func:`subprocess.run`; a test supplies its own so the probe boundary
        is exercised without executing a downloaded file.

    Returns
    -------
    int
        One of the exit codes stated in the module docstring.

    Examples
    --------
    A rehearsal identity records no tag, so planning against it is refused
    before any download is named. The diagnosis goes to standard error, which a
    doctest does not compare:

    >>> import json, tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     root = Path(tmp)
    ...     identity = root / "identity.json"
    ...     record = {
    ...         "crate": "netsuke-build-1.2.3.crate",
    ...         "identity": {
    ...             "tag": None, "commit": "0" * 40,
    ...             "package_name": "netsuke-build", "version": "1.2.3",
    ...             "bin_name": "netsuke", "is_prerelease": False,
    ...         },
    ...         "sha256": "ab" * 32,
    ...     }
    ...     _ = identity.write_text(json.dumps(record), encoding="utf-8")
    ...     trusted = root / "trusted.json"
    ...     _ = trusted.write_text(
    ...         json.dumps({"assets": {"a": "ab" * 32}}), encoding="utf-8"
    ...     )
    ...     staging = root / "staging.toml"
    ...     _ = staging.write_text(
    ...         '[targets.linux]' + chr(10)
    ...         + 'target = "x86_64-unknown-linux-gnu"' + chr(10),
    ...         encoding="utf-8",
    ...     )
    ...     manifest = root / "Cargo.toml"
    ...     _ = manifest.write_text(
    ...         '[package]' + chr(10) + 'name = "netsuke-build"' + chr(10),
    ...         encoding="utf-8",
    ...     )
    ...     main([
    ...         "plan",
    ...         "--identity", str(identity),
    ...         "--trusted", str(trusted),
    ...         "--staging-config", str(staging),
    ...         "--manifest", str(manifest),
    ...         "--repository", "leynos/netsuke",
    ...         "--target", "x86_64-unknown-linux-gnu",
    ...         "--commit", "0" * 40,
    ...         "--output", str(root / "plan.json"),
    ...     ])
    1
    """
    arguments = build_parser().parse_args(argv)
    if arguments.command == "plan":
        return run_plan(arguments)
    return verify.run_verify(arguments, runner=runner)


if __name__ == "__main__":
    raise SystemExit(main())
