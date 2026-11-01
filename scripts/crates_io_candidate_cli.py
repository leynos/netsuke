"""Validate the retained crate candidate and derive crates.io upload metadata.

This adapter reads the identity artifact and `.crate` bytes produced by
``package-crate``. It never invokes Cargo or rewrites the archive. The
registry policy remains in :mod:`publish_crates_io_cli` and its pure policy
modules; this module owns JSON, TOML, tar and filesystem boundaries.
"""

import argparse
import dataclasses
import enum
import io
import json
import pathlib
import sys
import tarfile
import tempfile
import tomllib
import typing as typ

import crates_io_upload_metadata
import release_asset_names
import release_candidate
import release_digest
import release_digest_files
import release_identity_record

EXIT_OK = 0
EXIT_REJECTED = 1
EXIT_OUTPUT_ERROR = 2
EXIT_USAGE = 64

SHA256_DIGEST_LENGTH = 64


class CandidateIssue(enum.StrEnum):
    """Classify failures while checking the retained release candidate."""

    JSON_READ = "could not read {} {}: {}"
    JSON_OBJECT = "{} must be a JSON object"
    REHEARSAL = "a rehearsal identity cannot publish to crates.io"
    IDENTITY_MISMATCH = "candidate identity mismatch: {}"
    CRATE_BASENAME = "identity record crate must be a flat archive basename"
    CRATE_SUFFIX = "identity record crate {!r} must end in .crate"
    ARCHIVE_COUNT = "expected exactly one matching .crate named {!r}, found {!r}"
    ARCHIVE_FILE = "candidate archive is not a regular file: {!r}"
    MANIFEST_MEMBER = "archive must contain exactly one regular {!r}"
    MANIFEST_READ = "could not read packaged manifest {!r}"
    MANIFEST_PARSE = "could not read packaged Cargo manifest: {}"
    PACKAGE_TABLE = "packaged manifest has no [package] table"
    MANIFEST_IDENTITY = (
        "packaged manifest identity does not match the validated candidate"
    )
    IDENTITY_OBJECT = "package identity record has no identity object"
    IDENTITY_INVALID = "invalid package identity: {}"
    DIGEST_MISSING = "package identity record has no SHA-256 digest"
    DIGEST_FORMAT = "package identity SHA-256 must be lowercase hexadecimal"
    DIGEST_MISMATCH = "candidate archive digest {} disagrees with identity record {}"


class CandidateError(ValueError):
    """Carry a classified candidate error and its display values."""

    def __init__(self, issue: CandidateIssue, context: tuple[object, ...] = ()) -> None:
        """Store the candidate issue and values used to render its diagnostic."""
        self.issue = issue
        self.context = context
        super().__init__(str(self))

    @typ.override
    def __str__(self) -> str:
        """Render the issue with its associated diagnostic values."""
        return self.issue.value.format(*self.context)


@dataclasses.dataclass(frozen=True, slots=True)
class CandidateExpectation:
    """Carry workflow identity that the package artifact must match."""

    tag: str
    commit: str
    package_name: str
    version: str


@dataclasses.dataclass(frozen=True, slots=True)
class ValidatedCandidate:
    """Carry the validated identity, archive path and archive digest."""

    identity: release_candidate.ReleaseIdentity
    crate: str
    archive: pathlib.Path
    sha256: str

    def as_record(self) -> dict[str, object]:
        """Return the candidate values in the action's JSON hand-off format.

        Returns
        -------
        dict[str, object]
            The JSON-compatible record consumed by the composite action.

        Examples
        --------
        >>> candidate = ValidatedCandidate(
        ...     release_candidate.ReleaseIdentity(
        ...         "v1.0.0", "0" * 40, "crate", "1.0.0", "crate", False
        ...     ), "crate-1.0.0.crate", pathlib.Path("crate-1.0.0.crate"), "ab"
        ... )
        >>> sorted(candidate.as_record())
        ['archive', 'crate', 'identity', 'sha256']
        """
        return {
            "archive": str(self.archive),
            "crate": self.crate,
            "identity": self.identity.as_record(),
            "sha256": self.sha256,
        }


def _read_json_mapping(path: pathlib.Path, label: str) -> dict[str, object]:
    """Read one JSON object, rejecting non-object and malformed records."""
    try:
        parsed = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise CandidateError(CandidateIssue.JSON_READ, (label, path, error)) from error
    if not isinstance(parsed, dict):
        raise CandidateError(CandidateIssue.JSON_OBJECT, (label,))
    return parsed


def _identity_matches(
    identity: release_candidate.ReleaseIdentity,
    expectation: CandidateExpectation,
) -> None:
    """Require the artifact identity to match this validated release run."""
    if identity.tag is None:
        raise CandidateError(CandidateIssue.REHEARSAL)
    expected = {
        "tag": expectation.tag,
        "commit": expectation.commit,
        "package_name": expectation.package_name,
        "version": expectation.version,
    }
    actual = {
        "tag": identity.tag,
        "commit": identity.commit,
        "package_name": identity.package_name,
        "version": identity.version,
    }
    mismatches = [
        f"{field}={actual[field]!r} (expected {value!r})"
        for field, value in expected.items()
        if actual[field] != value
    ]
    if mismatches:
        raise CandidateError(CandidateIssue.IDENTITY_MISMATCH, (", ".join(mismatches),))


def _recorded_archive(
    record: dict[str, object], directory: pathlib.Path
) -> tuple[str, pathlib.Path]:
    """Validate the archive basename before joining it to the artifact path."""
    crate = record.get("crate")
    if not isinstance(crate, str) or not release_asset_names.is_bare_file_name(crate):
        raise CandidateError(CandidateIssue.CRATE_BASENAME)
    if not crate.endswith(".crate"):
        raise CandidateError(CandidateIssue.CRATE_SUFFIX, (crate,))
    archives = sorted(directory.glob("*.crate"))
    if len(archives) != 1 or archives[0].name != crate:
        found = [archive.name for archive in archives]
        raise CandidateError(CandidateIssue.ARCHIVE_COUNT, (crate, found))
    archive = directory / crate
    if archive.is_symlink() or not archive.is_file():
        raise CandidateError(CandidateIssue.ARCHIVE_FILE, (crate,))
    return crate, archive


def _packaged_manifest(
    archive_path: pathlib.Path, package_name: str, version: str
) -> dict[str, object]:
    """Read and validate the Cargo manifest stored in the retained archive."""
    member_name = f"{package_name}-{version}/Cargo.toml"
    try:
        archive_bytes = archive_path.read_bytes()
        with tarfile.open(fileobj=io.BytesIO(archive_bytes), mode="r:gz") as package:
            matches = [
                member for member in package.getmembers() if member.name == member_name
            ]
            if len(matches) != 1 or not matches[0].isfile():
                raise CandidateError(CandidateIssue.MANIFEST_MEMBER, (member_name,))
            manifest_file = package.extractfile(matches[0])
            if manifest_file is None:
                raise CandidateError(CandidateIssue.MANIFEST_READ, (member_name,))
            manifest = tomllib.loads(manifest_file.read().decode("utf-8"))
    except (OSError, UnicodeError, tarfile.TarError, tomllib.TOMLDecodeError) as error:
        raise CandidateError(CandidateIssue.MANIFEST_PARSE, (error,)) from error
    package_table = manifest.get("package")
    if not isinstance(package_table, dict):
        raise CandidateError(CandidateIssue.PACKAGE_TABLE)
    if (
        package_table.get("name") != package_name
        or package_table.get("version") != version
    ):
        raise CandidateError(CandidateIssue.MANIFEST_IDENTITY)
    return manifest


def validate_candidate(
    identity_path: pathlib.Path,
    archive_directory: pathlib.Path,
    expectation: CandidateExpectation,
) -> ValidatedCandidate:
    """Validate identity, artifact selection, digest and packaged manifest.

    Returns
    -------
    ValidatedCandidate
        The identity and exact archive path and digest after validation.

    Raises
    ------
    CandidateError
        If the identity is a rehearsal, disagrees with the workflow, or does
        not vouch for the one flat archive and its packaged manifest.

    Examples
    --------
    >>> validate_candidate.__name__
    'validate_candidate'
    """
    record = _read_json_mapping(identity_path, "package identity")
    raw_identity = record.get("identity")
    if not isinstance(raw_identity, dict):
        raise CandidateError(CandidateIssue.IDENTITY_OBJECT)
    try:
        identity = release_identity_record.identity_from_mapping(raw_identity)
    except release_candidate.IdentityError as error:
        raise CandidateError(CandidateIssue.IDENTITY_INVALID, (error,)) from error
    _identity_matches(identity, expectation)
    crate, archive = _recorded_archive(record, archive_directory)
    expected_digest = record.get("sha256")
    if (
        not isinstance(expected_digest, str)
        or len(expected_digest) != SHA256_DIGEST_LENGTH
    ):
        raise CandidateError(CandidateIssue.DIGEST_MISSING)
    if any(character not in "0123456789abcdef" for character in expected_digest):
        raise CandidateError(CandidateIssue.DIGEST_FORMAT)
    observed_digest = release_digest_files.digest_of_file(archive)
    if not release_digest.constant_time_equal(observed_digest, expected_digest):
        raise CandidateError(
            CandidateIssue.DIGEST_MISMATCH, (observed_digest, expected_digest)
        )
    _ = _packaged_manifest(archive, expectation.package_name, expectation.version)
    return ValidatedCandidate(identity, crate, archive, observed_digest)


def _write_json(output: pathlib.Path, record: dict[str, object]) -> None:
    """Atomically install one JSON record beside the requested output."""
    temporary: pathlib.Path | None = None
    try:
        with tempfile.NamedTemporaryFile(
            mode="w",
            encoding="utf-8",
            dir=output.parent,
            prefix=f".{output.name}.",
            suffix=".tmp",
            delete=False,
        ) as stream:
            temporary = pathlib.Path(stream.name)
            _ = stream.write(json.dumps(record, indent=2, sort_keys=True) + "\n")
        temporary.replace(output)
    except OSError:
        if temporary is not None:
            temporary.unlink(missing_ok=True)
        raise


def build_parser() -> argparse.ArgumentParser:
    """Build the candidate check and upload-metadata command parser."""
    parser = argparse.ArgumentParser(
        prog="crates_io_candidate_cli",
        description="Validate packaged crate bytes and derive registry metadata.",
    )
    commands = parser.add_subparsers(dest="command", required=True)
    check = commands.add_parser("check", help="validate the packaged candidate")
    check.add_argument("--identity", required=True, type=pathlib.Path)
    check.add_argument("--archive-directory", required=True, type=pathlib.Path)
    check.add_argument("--tag", required=True)
    check.add_argument("--commit", required=True)
    check.add_argument("--package-name", required=True)
    check.add_argument("--version", required=True)
    check.add_argument("--output", required=True, type=pathlib.Path)
    metadata = commands.add_parser("metadata", help="derive Cargo registry metadata")
    metadata.add_argument("--archive", required=True, type=pathlib.Path)
    metadata.add_argument("--output", required=True, type=pathlib.Path)
    return parser


def main(argv: list[str] | None = None) -> int:
    """Run candidate validation or metadata derivation and return its status."""
    try:
        arguments = build_parser().parse_args(argv)
    except SystemExit as error:
        return EXIT_OK if error.code == 0 else EXIT_USAGE
    try:
        if arguments.command == "check":
            candidate = validate_candidate(
                arguments.identity,
                arguments.archive_directory,
                CandidateExpectation(
                    arguments.tag,
                    arguments.commit,
                    arguments.package_name,
                    arguments.version,
                ),
            )
            record = candidate.as_record()
        else:
            record = crates_io_upload_metadata.package_metadata(arguments.archive)
        _write_json(arguments.output, record)
    except (CandidateError, crates_io_upload_metadata.MetadataError) as error:
        print(f"crates.io candidate rejected: {error}", file=sys.stderr)
        return EXIT_REJECTED
    except OSError as error:
        print(f"crates.io candidate output failed: {error}", file=sys.stderr)
        return EXIT_OUTPUT_ERROR
    return EXIT_OK


if __name__ == "__main__":
    raise SystemExit(main())
