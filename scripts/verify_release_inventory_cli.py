"""Compare a published release's assets against the identity it was built from.

Two separate agreements are checked, and they fail for different reasons, so
each gets its own diagnosis:

1. The staged directory holds exactly the declared archives and sidecars. This
   is :func:`release_inventory.validate_inventory`, and it is the check that
   would have caught the 2026-10-12 releases, which carried six sidecars and
   none of the archives they described.
2. Every staged file's bytes match the digest its sidecar records. A sidecar
   is only evidence when it is checked against the file beside it; otherwise it
   is a claim about bytes nobody read.

The verifier reaches no network. The workflow step downloads the assets and
hands this module a directory, which keeps the whole comparison testable
offline and keeps the network boundary at the place holding the credential.
"""

import argparse
import json
import pathlib
import sys

import release_digest
import release_inventory

EXIT_OK = 0
EXIT_INVENTORY_REJECTED = 1
EXIT_INPUT_UNREADABLE = 2


def verify(directory: pathlib.Path, *, expected: list[str]) -> dict[str, str]:
    """Verify the inventory and every sidecar, returning the digests read.

    Parameters
    ----------
    directory
        Staging directory holding the release's assets.
    expected
        Asset names the release declared.

    Returns
    -------
    dict[str, str]
        Asset name to its verified bare hexadecimal digest, sorted by name.

    Notes
    -----
    Failures propagate from the policy modules rather than being translated
    here: :class:`release_inventory.InventoryError` when the staged set
    disagrees with the declaration, and :class:`release_digest.DigestError`
    when a sidecar is malformed or describes different bytes. Both stop the
    release, and :func:`main` is where they are turned into exit codes.

    Archives and sidecars are verified in one pass rather than sidecars alone.
    Verifying a sidecar against its archive already proves both, and taking the
    digest of an archive with no sidecar would leave that archive unverifiable
    by a consumer — so the inventory check runs first and the digest pass then
    assumes every name has a partner.

    The result is keyed by name and holds every declared asset, including the
    sidecars. The finalizer compares exactly this mapping against what GitHub
    reports, so producing it in the shape that comparison consumes keeps the
    two stages from disagreeing about the release's identity.

    Each entry is the digest of that asset's **own** bytes. A sidecar proves
    the archive beside it, but the release also publishes the sidecar, and
    GitHub reports a digest for the sidecar file just as it does for every
    other upload. Recording the archive's digest under the sidecar's name would
    make the finalizer compare the wrong value and either refuse a correct
    release or accept a sidecar that had been swapped after upload.
    """
    present = release_inventory.read_inventory(directory)
    release_inventory.validate_inventory(present, expected=expected)
    verified: dict[str, str] = {}
    for asset in sorted(present, key=lambda entry: entry.name):
        path = directory / asset.name
        if asset.name.endswith(release_inventory.SIDECAR_SUFFIX):
            archive = path.with_name(
                asset.name[: -len(release_inventory.SIDECAR_SUFFIX)]
            )
            # Verification only: the digest returned describes the archive.
            _ = release_digest.verify_file_against_sidecar(archive)
        verified[asset.name] = release_digest.digest_of_file(path)
    return verified


def build_parser() -> argparse.ArgumentParser:
    """Build the parser for the inventory-verification CLI.

    Returns
    -------
    argparse.ArgumentParser
        A parser requiring the staging directory, the declared names, and a
        destination for the record.
    """
    parser = argparse.ArgumentParser(
        prog="verify_release_inventory_cli",
        description="Verify a published release's assets against its declared set.",
    )
    parser.add_argument(
        "--directory",
        required=True,
        type=pathlib.Path,
        help="the directory holding the downloaded release assets",
    )
    parser.add_argument(
        "--expected",
        required=True,
        type=pathlib.Path,
        help="the file listing the declared asset names, one per line",
    )
    parser.add_argument(
        "--output",
        required=True,
        type=pathlib.Path,
        help="where to write the verified digest record",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    """Verify the inventory and write the digests that were confirmed.

    Parameters
    ----------
    argv
        Command-line arguments. Defaults to ``sys.argv[1:]``.

    Returns
    -------
    int
        ``EXIT_OK`` when every declared asset is present and matches its
        sidecar, ``EXIT_INVENTORY_REJECTED`` when the assets disagree with the
        declaration, or ``EXIT_INPUT_UNREADABLE`` when the declaration itself
        could not be read.

    Notes
    -----
    The declared names are read from a file rather than repeated as arguments.
    A release declares several dozen assets, and a command line that long is
    one an operator cannot compare against anything; the file is the same list
    the packaging stage produced.

    An unreadable declaration is reported separately from a rejected
    inventory. Both stop the release, but they send an operator to different
    places: one to the download that failed, the other to a path that was
    wrong before any asset was compared.

    Examples
    --------
    An unreadable declaration is refused before any asset is compared, and no
    record is written. The diagnosis goes to standard error, which a doctest
    does not compare:

    >>> main(["--directory", "/nonexistent", "--expected", "/nonexistent",
    ...       "--output", "/nonexistent/record.json"])
    2
    """
    arguments = build_parser().parse_args(argv)
    try:
        declared_text = arguments.expected.read_text(encoding="utf-8")
    except OSError as error:
        print(f"declaration unreadable: {error}", file=sys.stderr)
        return EXIT_INPUT_UNREADABLE
    expected = [line.strip() for line in declared_text.splitlines() if line.strip()]
    try:
        verified = verify(arguments.directory, expected=expected)
    except (release_inventory.InventoryError, release_digest.DigestError) as error:
        print(f"release inventory rejected: {error}", file=sys.stderr)
        return EXIT_INVENTORY_REJECTED
    arguments.output.write_text(
        json.dumps({"assets": verified}, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    return EXIT_OK


if __name__ == "__main__":
    raise SystemExit(main())
