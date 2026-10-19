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

Nothing here reaches the network. Every job that runs this has already
downloaded the artefacts; this module lists what should be there.

Run via ``scripts/release_declaration_cli.py`` in the release workflow;
behavioural coverage lives in ``scripts/tests/test_release_declaration_cli.py``.
"""

import argparse
import pathlib
import sys

import release_declaration

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

    Notes
    -----
    The two failures are separated because they send an operator to different
    places. ``EXIT_MISSING_ASSETS`` means the build promised a package it did
    not stage, and the next thing to read is that platform's job.
    ``EXIT_INPUT_UNUSABLE`` means the configuration or manifest was wrong
    before any asset was considered, so no build job is at fault.

    The declaration is written only on success. A partial file would let a
    later stage compare against a shorter list and call the release verified,
    which is the defect this module exists to make impossible.

    Examples
    --------
    An unreadable manifest is refused before any asset is considered, and no
    declaration is written. The diagnosis goes to standard error, which a
    doctest does not compare:

    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     output = Path(tmp) / "declared.txt"
    ...     main(["--dist-dir", tmp, "--staging-config", str(Path(tmp) / "s.toml"),
    ...           "--manifest", str(Path(tmp) / "absent.Cargo.toml"),
    ...           "--version", "1.0.0", "--output", str(output)])
    2
    """
    arguments = build_parser().parse_args(argv)
    try:
        names = release_declaration.declared_asset_names(
            arguments.dist_dir,
            arguments.staging_config,
            arguments.manifest,
            arguments.version,
        )
    except release_declaration.ConfigurationError as error:
        print(f"release declaration inputs unusable: {error}", file=sys.stderr)
        return EXIT_INPUT_UNUSABLE
    except release_declaration.StagedTreeError as error:
        print(f"release declaration refused: {error}", file=sys.stderr)
        return EXIT_MISSING_ASSETS
    except (OSError, KeyError, ValueError) as error:
        print(f"release declaration inputs unreadable: {error}", file=sys.stderr)
        return EXIT_INPUT_UNUSABLE
    release_declaration.write_declaration(names, arguments.output)
    return EXIT_OK


if __name__ == "__main__":
    raise SystemExit(main())
