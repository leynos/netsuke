"""Compare a published release's assets against the identity it was built from.

Three separate agreements are checked, and they fail for different reasons, so
each gets its own diagnosis:

1. The staged directory holds exactly the declared archives and sidecars. This
   is :func:`release_inventory.validate_inventory`, and it is the check that
   would have caught the 2026-10-12 releases, which carried six sidecars and
   none of the archives they described.
2. Every staged file's bytes match the digest its sidecar records. A sidecar
   is only evidence when it is checked against the file beside it; otherwise it
   is a claim about bytes nobody read.
3. Every staged file's bytes match the digest the build recorded for the staged
   candidate before upload. This is the trusted comparison, and it is the only
   one of the three that a replaced asset cannot satisfy.

The third agreement is what makes the other two worth making. A sidecar travels
with its archive, so replacing both together leaves agreement 2 intact and the
release self-consistent — the download would vouch for itself. Agreement 3 is
anchored outside the download, in a record produced from the build's own tree,
so a substituted operating-system package or a replaced archive/sidecar pair
both fail it. ``--trusted`` names that record, and it is written by
``release_declaration_cli --digest-record`` before the upload step runs.

The verifier reaches no network. The workflow step downloads the assets and
hands this module a directory, which keeps the whole comparison testable
offline and keeps the network boundary at the place holding the credential.
"""

import argparse
import json
import pathlib
import sys
import typing as typ

import release_digest
import release_digest_record
import release_inventory

if typ.TYPE_CHECKING:
    import collections.abc as cabc

EXIT_OK = 0
EXIT_INVENTORY_REJECTED = 1
EXIT_INPUT_UNREADABLE = 2


def verify(
    directory: pathlib.Path,
    *,
    expected: list[str],
    trusted: cabc.Mapping[str, str],
) -> dict[str, str]:
    """Verify the inventory, every sidecar, and every trusted digest.

    Parameters
    ----------
    directory
        Staging directory holding the release's assets.
    expected
        Asset names the release declared.
    trusted
        Published name to the digest the build recorded for the staged
        candidate before upload.

    Returns
    -------
    dict[str, str]
        Asset name to its bare hexadecimal digest, sorted by name.

    Notes
    -----
    Four refusals reach the caller, one per agreement plus the record's own
    coverage check, and each is a distinct type so :func:`main` can report it
    without parsing a message: :class:`release_inventory.InventoryError` when
    the staged set disagrees with the declaration,
    :class:`release_digest.DigestError` when a sidecar is malformed or
    describes different bytes than the archive beside it,
    :class:`release_digest_record.DigestRecordError` when the trusted record
    does not cover exactly the declared assets, and
    :class:`release_digest_record.RecordedDigestMismatchError` when an asset's
    bytes differ from the digest recorded for it.

    All four propagate from the policy modules rather than being translated
    here, so :func:`main` is the single place they become exit codes. That is
    also why none appears in a ``Raises`` section: nothing in this function
    raises one, and Ruff's DOC501 reserves that section for exceptions the
    function raises itself.

    The trusted record is checked against the declaration before any download
    is hashed. A record that misses a declared asset would leave that asset
    uncompared, so there is no useful work to do on the files until the record
    is known to cover the release.

    Archives and sidecars are verified in one pass rather than sidecars alone.
    Verifying a sidecar against its archive already proves both, and taking the
    digest of an archive with no sidecar would leave that archive unverifiable
    by a consumer — so the inventory check runs first and the digest pass then
    assumes every name has a partner.

    The result holds every asset's **own** digest, sidecars included. A sidecar
    proves the archive beside it, but the release publishes the sidecar too, so
    recording the archive's digest under the sidecar's name would make the
    finalizer compare the wrong value.
    """
    present = release_inventory.read_inventory(directory)
    release_inventory.validate_inventory(present, expected=expected)
    release_digest_record.require_record_matches_declaration(trusted, expected=expected)
    computed = _digests_of_downloads(directory, present)
    release_digest_record.require_downloads_match_record(computed, trusted=trusted)
    return computed


def _digests_of_downloads(
    directory: pathlib.Path,
    present: cabc.Iterable[release_inventory.PublishedAsset],
) -> dict[str, str]:
    """Hash each downloaded asset and check every sidecar against its archive.

    Returns
    -------
    dict[str, str]
        Asset name to the digest of that asset's own bytes, sorted by name.

    Notes
    -----
    A sidecar is verified against its neighbour *and* hashed in its own right.
    The first proves the archive, the second proves the sidecar file that is
    also published, and skipping either leaves one of the two able to be
    swapped unnoticed.
    """
    computed: dict[str, str] = {}
    for asset in sorted(present, key=lambda entry: entry.name):
        path = directory / asset.name
        if asset.name.endswith(release_inventory.SIDECAR_SUFFIX):
            archive = path.with_name(
                asset.name[: -len(release_inventory.SIDECAR_SUFFIX)]
            )
            # Verification only: the digest returned describes the archive.
            _ = release_digest.verify_file_against_sidecar(archive)
        computed[asset.name] = release_digest.digest_of_file(path)
    return computed


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
        "--trusted",
        required=True,
        type=pathlib.Path,
        help=(
            "the digest record the build wrote for the staged candidate before "
            "upload; every downloaded asset is compared against it"
        ),
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
        ``EXIT_OK`` when every declared asset is present, matches its sidecar,
        and matches the digest the build recorded before upload;
        ``EXIT_INVENTORY_REJECTED`` when any of those agreements fails; or
        ``EXIT_INPUT_UNREADABLE`` when the declaration or the trusted record
        could not be read at all.

    Notes
    -----
    The declared names are read from a file rather than repeated as arguments.
    A release declares several dozen assets, and a command line that long is
    one an operator cannot compare against anything; the file is the same list
    the packaging stage produced.

    An unreadable input is reported separately from a rejected inventory. Both
    stop the release, but they send an operator to different places: one to the
    download that failed, the other to a path that was wrong before any asset
    was compared.

    A *malformed* trusted record is different again, and it is rejected rather
    than treated as unreadable. The distinction is deliberate: an unreadable
    path means the workflow wired the wrong file, while a record that parses
    and is structurally wrong means the build wrote something the verifier
    cannot trust. Only the first is an operator's typo.

    Examples
    --------
    An unreadable declaration is refused before any asset is compared, and no
    record is written. The diagnosis goes to standard error, which a doctest
    does not compare:

    >>> main(["--directory", "/nonexistent", "--expected", "/nonexistent",
    ...       "--trusted", "/nonexistent/trusted.json",
    ...       "--output", "/nonexistent/record.json"])
    2
    """
    arguments = build_parser().parse_args(argv)
    try:
        declared_text = arguments.expected.read_text(encoding="utf-8")
        trusted = release_digest_record.read_digest_record(arguments.trusted)
    except OSError as error:
        print(f"declaration unreadable: {error}", file=sys.stderr)
        return EXIT_INPUT_UNREADABLE
    except release_digest_record.DigestRecordError as error:
        print(f"trusted digest record rejected: {error}", file=sys.stderr)
        return EXIT_INVENTORY_REJECTED
    expected = [line.strip() for line in declared_text.splitlines() if line.strip()]
    try:
        verified = verify(arguments.directory, expected=expected, trusted=trusted)
    except (
        release_inventory.InventoryError,
        release_digest.DigestError,
        release_digest_record.DigestRecordError,
    ) as error:
        print(f"release inventory rejected: {error}", file=sys.stderr)
        return EXIT_INVENTORY_REJECTED
    arguments.output.write_text(
        json.dumps({"assets": verified}, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    return EXIT_OK


if __name__ == "__main__":
    raise SystemExit(main())
