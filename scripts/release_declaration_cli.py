"""Write the release's declared asset list for the publication jobs to read.

The upload step, the inventory verifier and the finalizer all need the same
answer to "which assets does this release publish?". This CLI is where that
answer is produced: it calls :func:`release_declaration.declared_asset_names`
and writes the result as one name per line, which is the format
``verify_release_inventory_cli --expected`` already reads.

Producing the declaration and comparing against it stay separate steps, and
that separation is the point. A producer that read the staged tree and declared
whatever it found would make the verifier's exact-set rule vacuous: the release
would be checked against itself. Here the archive half comes from configuration
and the operating-system half is held to a per-platform count, so both halves
can disagree with the tree.

The same run also writes the trusted digest record, because this is the only
stage holding both the declared names and the staged bytes, and it is the last
point before upload at which the bytes are still the build's own. The verifier
compares the download against that record, so a substituted package or a
replaced archive/sidecar pair is caught. Both files are written only on
success; a partial record would be worse than none, because it would read as
permission to verify less.

Nothing here reaches the network. Every job that runs this has already
downloaded the artefacts; this module lists what should be there.

Run via ``scripts/release_declaration_cli.py`` in the release workflow;
behavioural coverage lives in ``scripts/tests/test_release_declaration_cli.py``.
"""

import argparse
import pathlib
import sys
import tomllib

import hoist_binstall_discovery
import release_declaration
import release_digest_record_files
import release_github_assets_adapter
import release_staged_digests

EXIT_OK = 0
EXIT_MISSING_ASSETS = 1
EXIT_INPUT_UNUSABLE = 2


def build_parser() -> argparse.ArgumentParser:
    """Build the parser for the declared-name producer.

    Returns
    -------
    argparse.ArgumentParser
        A parser requiring the staging directory, the configuration and
        manifest the names derive from, the release version, and the
        destination for the declaration.
    """
    parser = argparse.ArgumentParser(
        prog="release_declaration_cli",
        description="Declare the asset names a correct release publishes.",
    )
    parser.add_argument(
        "--dist-dir",
        required=True,
        type=pathlib.Path,
        help="the staging directory holding the downloaded release artefacts",
    )
    parser.add_argument(
        "--staging-config",
        required=True,
        type=pathlib.Path,
        help="the release staging configuration naming the targets and platforms",
    )
    parser.add_argument(
        "--manifest",
        required=True,
        type=pathlib.Path,
        help="the Cargo manifest naming the package",
    )
    parser.add_argument(
        "--version",
        required=True,
        help="the release version interpolated into each archive name",
    )
    parser.add_argument(
        "--output",
        required=True,
        type=pathlib.Path,
        help="where to write the declared asset names, one per line",
    )
    parser.add_argument(
        "--digest-record",
        required=True,
        type=pathlib.Path,
        help=(
            "where to write the trusted digest record the inventory verifier "
            "compares the download against"
        ),
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    """Declare the release's asset names, or report why they cannot be.

    Parameters
    ----------
    argv
        Command-line arguments. Defaults to ``sys.argv[1:]``.

    Returns
    -------
    int
        ``EXIT_OK`` when the declaration was written,
        ``EXIT_MISSING_ASSETS`` when the staged tree disagrees with the
        configured targets and formats, or ``EXIT_INPUT_UNUSABLE`` when a
        configuration or manifest could not be read or does not describe a
        releasable build.

    Examples
    --------
    An unreadable manifest is refused before any asset is considered, and no
    declaration is written. The diagnosis goes to standard error, which a
    doctest does not compare:

    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     output = Path(tmp) / "declared.txt"
    ...     record = Path(tmp) / "record.json"
    ...     main(["--dist-dir", tmp, "--staging-config", str(Path(tmp) / "s.toml"),
    ...           "--manifest", str(Path(tmp) / "absent.Cargo.toml"),
    ...           "--version", "1.0.0", "--output", str(output),
    ...           "--digest-record", str(record)])
    2
    """
    arguments = build_parser().parse_args(argv)
    try:
        names, digests = _compute_declaration(arguments)
    except release_declaration.ConfigurationError as error:
        print(f"release declaration inputs unusable: {error}", file=sys.stderr)
        return EXIT_INPUT_UNUSABLE
    except release_declaration.StagedTreeError as error:
        print(f"release declaration refused: {error}", file=sys.stderr)
        return EXIT_MISSING_ASSETS
    except (OSError, KeyError, ValueError) as error:
        print(f"release declaration inputs unreadable: {error}", file=sys.stderr)
        return EXIT_INPUT_UNUSABLE
    arguments.output.write_text(
        "".join(f"{name}\n" for name in names), encoding="utf-8"
    )
    release_digest_record_files.write_digest_record(digests, arguments.digest_record)
    return EXIT_OK


def _compute_declaration(
    arguments: argparse.Namespace,
) -> tuple[tuple[str, ...], dict[str, str]]:
    """Compute the declared names and trusted digests from parsed CLI inputs.

    Returns
    -------
    tuple[tuple[str, ...], dict[str, str]]
        The sorted names and their lowercase SHA-256 digests.

    Examples
    --------
    The configuration is parsed before any declaration can be written:

    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     config = Path(tmp) / "staging.toml"
    ...     _ = config.write_text("invalid = [", encoding="utf-8")
    ...     args = argparse.Namespace(
    ...         staging_config=config, manifest=Path(tmp) / "Cargo.toml",
    ...         version="1.0.0", dist_dir=Path(tmp)
    ...     )
    ...     try:
    ...         _compute_declaration(args)
    ...     except tomllib.TOMLDecodeError:
    ...         print("refused")
    refused
    """
    config = tomllib.loads(arguments.staging_config.read_text(encoding="utf-8"))
    archive_names = hoist_binstall_discovery.expected_archive_names(
        arguments.staging_config, arguments.manifest, arguments.version
    )
    staged_package_names = [
        release_github_assets_adapter.published_asset_name(path.name)
        for path in hoist_binstall_discovery.walk_files(arguments.dist_dir)
    ]
    names = release_declaration.declared_asset_names(
        archive_names=archive_names,
        target_config=config,
        staged_package_names=staged_package_names,
    )
    digests = release_staged_digests.staged_digests(arguments.dist_dir, expected=names)
    return names, digests


if __name__ == "__main__":
    raise SystemExit(main())
