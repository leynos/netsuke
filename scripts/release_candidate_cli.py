"""Record the release identity a packaging run is authorised to publish.

This is the seam between the workflow and :mod:`release_candidate`, which is a
pure policy module: it validates facts and never reads a file or a command
line. Keeping the argument parsing and the single write here leaves the policy
module inside the no-I/O contract its own tests prove, and gives the packaging
job one command to invoke.

The identity is written only after validation succeeds. A rejected tag,
version, or commit therefore leaves no record behind, so a later stage cannot
mistake a half-written identity for an authorised one.
"""

import argparse
import pathlib
import sys

import release_candidate

EXIT_OK = 0
EXIT_IDENTITY_REJECTED = 1


def build_parser() -> argparse.ArgumentParser:
    """Build the parser for the identity-recording CLI.

    Returns
    -------
    argparse.ArgumentParser
        A parser requiring every identity fact, because each one is checked
        against the others and a defaulted value would silently weaken the
        agreement rules.
    """
    parser = argparse.ArgumentParser(
        prog="release_candidate_cli",
        description="Validate one release candidate and write its identity record.",
    )
    parser.add_argument("--tag", required=True, help="the release tag, e.g. v1.2.3")
    parser.add_argument("--commit", required=True, help="the full commit SHA")
    parser.add_argument("--package-name", required=True, help="the Cargo package name")
    parser.add_argument("--version", required=True, help="the manifest version")
    parser.add_argument("--bin-name", required=True, help="the installed binary name")
    parser.add_argument(
        "--output",
        required=True,
        type=pathlib.Path,
        help="where to write the identity record",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    """Validate the candidate and write its identity record.

    Parameters
    ----------
    argv
        Command-line arguments. Defaults to ``sys.argv[1:]``.

    Returns
    -------
    int
        ``EXIT_OK`` when the identity was recorded, or
        ``EXIT_IDENTITY_REJECTED`` when validation refused the facts.

    Examples
    --------
    A tag that does not spell the manifest version is refused, and no record
    is written. The destination is deliberately a path that could not be
    opened, so an example that wrongly accepted the tag would raise here
    instead of quietly recording an identity. The diagnosis goes to standard
    error, which a doctest does not compare:

    >>> main(["--tag", "v2.0.0", "--commit", "0" * 40, "--package-name", "p",
    ...       "--version", "1.2.3", "--bin-name", "b",
    ...       "--output", "/nonexistent/identity.json"])
    1
    """
    arguments = build_parser().parse_args(argv)
    facts = release_candidate.CandidateFacts(
        tag=arguments.tag,
        commit=arguments.commit,
        package_name=arguments.package_name,
        version=arguments.version,
        bin_name=arguments.bin_name,
    )
    try:
        identity = release_candidate.resolve_identity(facts)
    except release_candidate.IdentityError as error:
        print(f"release identity rejected: {error}", file=sys.stderr)
        return EXIT_IDENTITY_REJECTED
    arguments.output.write_text(identity.to_json(), encoding="utf-8")
    return EXIT_OK


if __name__ == "__main__":
    raise SystemExit(main())
