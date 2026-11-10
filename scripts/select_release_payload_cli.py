"""Copy the declared release payload into a fresh directory for upload.

The upload step receives whatever the download and hoist steps left in
``dist``: one directory per producer, hoisted archives at the root, and the
build products that ship inside the packages. Uploading that tree would
publish names the release never declared. This CLI instead observes the tree,
asks :mod:`release_payload_selection` what the release declares, and copies
exactly those files -- under their published names -- into a directory the
uploader then treats as the whole payload.

The selection record written here is the selection's own view of the producers'
claims. The action retains it with the declaration record as a workflow
artefact, and the uploader reads it back as the independent account of what the
payload must contain: a staged tree replaced between the two hashes yields a
declaration record that faithfully describes the replacement, so only the two
records answering for one another close that window.

The inventory verifier and the release finalizer are written and their suites
pass, but neither job runs in the release workflow yet, so the action's own
upload step is the only consumer today. Both files are retained beyond the
upload because they are what will hand the same account on once those jobs are
wired.

Tree observation and the verified copy live in :mod:`release_payload_tree`, so
this module is the argument surface and nothing else.

Run via ``.github/actions/upload-declared-assets``; behavioural coverage lives
in ``scripts/tests/test_select_release_payload_cli.py``.
"""

import argparse
import pathlib
import sys
import tomllib

import hoist_binstall_discovery
import release_declaration
import release_digest_files
import release_package_origins
import release_payload_selection
import release_payload_tree
from release_payload_errors import PayloadError, StagedFileUnreadableError

# Re-exported so the workflow step and its tests reach the tree adapter through
# the CLI that composes it, without this module's own code depending on the
# aliases.
from release_payload_tree import (
    PAYLOAD_DIRECTORY,
    RECORD_NAME,
    observe_payload,
)
from release_producer_provenance_errors import (
    DuplicateAssetError,
    DuplicateTargetError,
    ForeignRunError,
    MissingTargetError,
    ProvenanceError,
    StaleCandidateError,
    TargetClaimMismatchError,
)

__all__ = [
    "EXIT_OK",
    "EXIT_OUTPUT_UNWRITABLE",
    "EXIT_PAYLOAD_REFUSED",
    "EXIT_UNUSABLE_INPUT",
    "PAYLOAD_DIRECTORY",
    "RECORD_NAME",
    "build_parser",
    "main",
    "observe_payload",
    "select_payload",
]

EXIT_OK = 0
EXIT_PAYLOAD_REFUSED = 1
EXIT_UNUSABLE_INPUT = 2
EXIT_OUTPUT_UNWRITABLE = 3


def build_parser() -> argparse.ArgumentParser:
    """Build the parser for the payload-selection CLI.

    Returns
    -------
    argparse.ArgumentParser
        A parser requiring the release identity, the configuration and
        manifest the declaration derives from, the staged tree, and the
        destinations for the payload and its trusted record.

    Examples
    --------
    >>> arguments = build_parser().parse_args([
    ...     "--dist-dir", "dist", "--staging-config", "s.toml",
    ...     "--manifest", "Cargo.toml", "--version", "1.0.0",
    ...     "--commit", "a" * 40, "--run-id", "7",
    ...     "--output-dir", "artifacts/payload",
    ...     "--digest-record", "artifacts/trusted.json",
    ... ])
    >>> arguments.run_id
    '7'
    """
    parser = argparse.ArgumentParser(
        prog="select_release_payload_cli",
        description="Select and copy the declared release payload for upload.",
    )
    parser.add_argument("--dist-dir", required=True, type=pathlib.Path)
    parser.add_argument("--staging-config", required=True, type=pathlib.Path)
    parser.add_argument("--manifest", required=True, type=pathlib.Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--run-id", required=True)
    parser.add_argument(
        "--output-dir",
        required=True,
        type=pathlib.Path,
        help="a directory that must not exist yet; the payload is created in it",
    )
    parser.add_argument(
        "--digest-record",
        required=True,
        type=pathlib.Path,
        help="where to write the trusted digest record produced from staged bytes",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    """Select the declared payload and copy it, or report why it was refused.

    Parameters
    ----------
    argv
        Command-line arguments. Defaults to ``sys.argv[1:]``.

    Returns
    -------
    int
        ``EXIT_OK`` when the payload and its record were written,
        ``EXIT_PAYLOAD_REFUSED`` when the tree is well formed but disagrees
        with the release, ``EXIT_UNUSABLE_INPUT`` when a configuration,
        manifest, or producer record cannot be read, or
        ``EXIT_OUTPUT_UNWRITABLE`` when the payload cannot be installed.

    Notes
    -----
    A producer record naming an unknown platform, carrying a malformed digest,
    or claiming no assets is *unusable input* rather than a refused payload:
    the record itself is broken, so re-running publication will not help until
    the producer is fixed.

    Examples
    --------
    A missing producer record is refused before anything is copied:

    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     dist = Path(tmp) / "dist"
    ...     dist.mkdir()
    ...     main(["--dist-dir", str(dist), "--staging-config", str(dist / "s"),
    ...           "--manifest", str(dist / "m"), "--version", "1.0.0",
    ...           "--commit", "a" * 40, "--run-id", "7",
    ...           "--output-dir", str(Path(tmp) / "out"),
    ...           "--digest-record", str(Path(tmp) / "record.json")])
    2
    """
    arguments = build_parser().parse_args(argv)
    try:
        selection = select_payload(arguments)
    except (OSError, KeyError, ValueError) as error:
        message, status = _selection_refusal(error)
        print(message, file=sys.stderr)
        return status
    try:
        release_payload_tree.install_payload(
            selection,
            arguments.output_dir,
            arguments.digest_record,
            _digests(selection),
        )
    except OSError as error:
        # Selection reads everything it judges, so an OSError from here is the
        # destination refusing the copy or the record: an output directory that
        # already exists, a full disk, a permission denial. That is
        # EXIT_OUTPUT_UNWRITABLE, and reporting it as unusable *input* would
        # send an operator to the producer records for a workspace fault.
        print(f"payload could not be installed: {error}", file=sys.stderr)
        return EXIT_OUTPUT_UNWRITABLE
    except StagedFileUnreadableError as error:
        print(f"payload inputs unreadable: {error}", file=sys.stderr)
        return EXIT_UNUSABLE_INPUT
    except PayloadError as error:
        # A source that stopped being a regular file between the digest check
        # and the copy, or a path that would smuggle bytes from outside the
        # download. Selection approved the names; the path is what changed.
        print(f"payload refused: {error}", file=sys.stderr)
        return EXIT_PAYLOAD_REFUSED
    print(f"selected {len(selection.declared)} declared assets")
    return EXIT_OK


def _selection_refusal(error: BaseException) -> tuple[str, int]:
    """Say what a selection failure means and which status reports it.

    Parameters
    ----------
    error
        The exception :func:`select_payload` raised.

    Returns
    -------
    tuple[str, int]
        The diagnostic to print and the exit status it implies.

    Notes
    -----
    The cases are ordered so a subclass is matched before its parent:
    ``StagedFileUnreadableError`` is a ``PayloadError``, and the six tree
    mismatches are ``ProvenanceError``. Reading a staged file whose mode the
    operating system would not describe as a wrong artefact would send an
    operator to the producer job; it is an environment fault, and the remedy
    is to re-run on a healthy runner.
    """
    match error:
        case StagedFileUnreadableError():
            return f"payload inputs unreadable: {error}", EXIT_UNUSABLE_INPUT
        # A bare name in a pattern is a *capture*, not a value lookup, so the
        # six tree mismatches are spelled out here rather than matched as one
        # tuple; a class pattern needs each class named.
        case (
            DuplicateAssetError()
            | DuplicateTargetError()
            | ForeignRunError()
            | MissingTargetError()
            | StaleCandidateError()
            | TargetClaimMismatchError()
        ):
            return f"payload refused: {error}", EXIT_PAYLOAD_REFUSED
        case ProvenanceError():
            # The record *itself* is broken -- an unknown platform, a
            # malformed digest, no assets at all. Re-running cannot repair it.
            return f"producer record unusable: {error}", EXIT_UNUSABLE_INPUT
        case PayloadError():
            return f"payload refused: {error}", EXIT_PAYLOAD_REFUSED
        case _:
            return f"payload inputs unreadable: {error}", EXIT_UNUSABLE_INPUT


def select_payload(
    arguments: argparse.Namespace,
) -> release_payload_selection.PayloadSelection[pathlib.Path]:
    """Decide the payload without touching the destination.

    Parameters
    ----------
    arguments
        Parsed command-line arguments, as :func:`build_parser` produces.

    Returns
    -------
    release_payload_selection.PayloadSelection
        The approved selection, holding only staged source paths.

    Notes
    -----
    Reads the configuration, the producer records and the staged tree, and
    reaches no destination. Every refusal a *tree* can provoke is raised
    here; a destination that will not accept the copy is
    :func:`release_payload_tree.install_payload`'s.
    """
    config = _read_staging_config(arguments.staging_config)
    # Read each record once: `record_locations` and the claim check must judge
    # the same bytes, or a record rewritten between two reads would attribute
    # a package to one target and describe it with another's claims.
    records = release_payload_tree.read_provenance_by_path(arguments.dist_dir)
    locations = release_payload_tree.record_locations(records)
    provenances = sorted(records.values(), key=lambda record: record.target)
    archives = hoist_binstall_discovery.archives_by_target(
        arguments.staging_config, arguments.manifest, arguments.version
    )
    staged = release_payload_tree.observe_payload(arguments.dist_dir)
    selection = release_payload_selection.select_payload(
        provenances,
        identity=release_payload_selection.ReleaseIdentity(
            candidate=arguments.commit,
            run_id=arguments.run_id,
            target_config=config,
        ),
        tree=release_payload_selection.StagedTree(
            archives=archives,
            staged=staged,
            package_origins=release_package_origins.observe_package_origins(
                arguments.dist_dir, target_of_record=locations
            ),
        ),
    )
    release_payload_selection.require_staged_digests(
        _digests(selection), trusted=selection.trusted
    )
    return selection


def _read_staging_config(config_path: pathlib.Path) -> dict[str, object]:
    """Parse and validate the staging configuration, refusing nothing here.

    Returns
    -------
    dict[str, object]
        The parsed configuration, already checked for usable target tables.

    Notes
    -----
    A malformed configuration surfaces as ``tomllib.TOMLDecodeError``, and an
    unusable one as ``ConfigurationError`` from
    :func:`release_declaration.target_platforms`.
    """
    config = tomllib.loads(config_path.read_text(encoding="utf-8"))
    _ = release_declaration.target_platforms(config)
    return config


def _digests(
    selection: release_payload_selection.PayloadSelection[pathlib.Path],
) -> dict[str, str]:
    """Hash each selected source under the name it will publish under.

    Returns
    -------
    dict[str, str]
        Published name mapped to the lowercase SHA-256 of its staged bytes.
    """
    return {
        name: release_digest_files.digest_of_file(selection.sources[name])
        for name in selection.declared
    }


if __name__ == "__main__":
    raise SystemExit(main())
