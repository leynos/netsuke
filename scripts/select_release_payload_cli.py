"""Copy the declared release payload into a fresh directory for upload.

The upload step receives whatever the download and hoist steps left in
``dist``: one directory per producer, hoisted archives at the root, and the
build products that ship inside the packages. Uploading that tree would
publish names the release never declared. This CLI instead observes the tree,
asks :mod:`release_payload_selection` what the release declares, and copies
exactly those files -- under their published names -- into a directory the
uploader then treats as the whole payload.

The trusted digest record written here is the selection's own view of the
producers' claims. It travels to the verifier and the finalizer, which is what
keeps the comparison anchored outside the downloaded tree.

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
import release_payload_selection
import release_payload_tree
from release_payload_errors import PayloadError

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
    "select_and_copy",
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
        selection = select_and_copy(arguments)
    except release_declaration.ConfigurationError as error:
        print(f"payload inputs unusable: {error}", file=sys.stderr)
        return EXIT_UNUSABLE_INPUT
    except (
        DuplicateAssetError,
        DuplicateTargetError,
        ForeignRunError,
        MissingTargetError,
        StaleCandidateError,
        TargetClaimMismatchError,
    ) as error:
        # These describe a tree that is *readable* but is not this release's:
        # a stale download, a foreign run, a missing or doubled producer. The
        # artefacts are wrong, so they are refused as a payload.
        print(f"payload refused: {error}", file=sys.stderr)
        return EXIT_PAYLOAD_REFUSED
    except ProvenanceError as error:
        # Everything else in the family says the record *itself* is broken --
        # an unknown platform, a malformed digest, no assets at all. Re-running
        # publication cannot repair that; the producer job must be fixed.
        print(f"producer record unusable: {error}", file=sys.stderr)
        return EXIT_UNUSABLE_INPUT
    except PayloadError as error:
        print(f"payload refused: {error}", file=sys.stderr)
        return EXIT_PAYLOAD_REFUSED
    except (OSError, KeyError, ValueError) as error:
        print(f"payload inputs unreadable: {error}", file=sys.stderr)
        return EXIT_UNUSABLE_INPUT
    print(f"selected {len(selection.declared)} declared assets")
    return EXIT_OK


def select_and_copy(
    arguments: argparse.Namespace,
) -> release_payload_selection.PayloadSelection[pathlib.Path]:
    """Select the payload, verify its digests, and copy it into place.

    Parameters
    ----------
    arguments
        Parsed command-line arguments, as :func:`build_parser` produces.

    Returns
    -------
    release_payload_selection.PayloadSelection
        The approved selection, with its sources as staged paths.

    Notes
    -----
    A tree that disagrees with the release, a malformed or foreign producer
    record, or staged bytes that differ from the record surface a
    :class:`release_payload_errors.PayloadError`; an input that cannot be read
    or a payload that cannot be installed surfaces ``OSError``. Every one of
    them reaches the caller from a helper below.

    The output directory is created only after every check has passed. It is
    removed again if copying fails, so a half-written payload is never left
    behind for the upload step to publish.
    """
    config = _read_staging_config(arguments.staging_config)
    provenances = release_payload_tree.read_provenances(arguments.dist_dir)
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
        archives=archives,
        staged=staged,
    )
    computed = _digests(selection)
    release_payload_selection.require_staged_digests(
        computed, trusted=selection.trusted
    )
    release_payload_tree.install_payload(
        selection, arguments.output_dir, arguments.digest_record, computed
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
