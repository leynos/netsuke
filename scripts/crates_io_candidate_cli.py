"""Validate the retained crate candidate and derive crates.io upload metadata.

This adapter reads the identity artifact and `.crate` bytes produced by
``package-crate``. It never invokes Cargo or rewrites the archive. The
registry policy remains in :mod:`publish_crates_io_cli` and its pure policy
modules; this module owns the JSON and filesystem boundaries, while
:mod:`crates_io_candidate_archive` owns reading the archive and the manifest
it packages.
"""

import argparse
import dataclasses
import json
import pathlib
import sys
import tempfile

import crates_io_candidate_archive
import crates_io_upload_metadata
import release_candidate
import release_digest
import release_digest_files
import release_identity_record
from crates_io_candidate_errors import CandidateError, CandidateIssue

EXIT_OK = 0
EXIT_REJECTED = 1
EXIT_OUTPUT_ERROR = 2
EXIT_USAGE = 64

SHA256_DIGEST_LENGTH = 64


@dataclasses.dataclass(frozen=True, slots=True)
class CandidateExpectation:
    """Carry workflow identity that the package artifact must match.

    ``tag`` is ``None`` for a rehearsal. A pull request has a merge ref rather
    than a release tag, so a rehearsed candidate records no tag and this check
    binds only the commit, package name, and version. A publishing run supplies
    the tag, and the identity must then record exactly that tag.
    """

    tag: str | None
    commit: str
    package_name: str
    version: str

    @property
    def is_rehearsal(self) -> bool:
        """Whether this expectation accepts a tagless candidate."""
        return self.tag is None


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


def _require_release_mode(
    identity: release_candidate.ReleaseIdentity,
    expectation: CandidateExpectation,
) -> None:
    """Require the recorded tag to agree with the run's release mode."""
    # A rehearsal and a publishing run are distinguished by the tag alone. A
    # rehearsal must record no tag, because inventing one would let a pull
    # request claim a release it never built. A publishing run must record the
    # tag it releases, because a candidate built for another tag is not
    # publishable here.
    if expectation.is_rehearsal:
        if identity.tag is not None:
            raise CandidateError(CandidateIssue.UNEXPECTED_TAG, (identity.tag,))
    elif identity.tag is None:
        raise CandidateError(CandidateIssue.REHEARSAL)


def _identity_matches(
    identity: release_candidate.ReleaseIdentity,
    expectation: CandidateExpectation,
) -> None:
    """Require the artifact identity to match this validated release run."""
    _require_release_mode(identity, expectation)
    expected = {
        "commit": expectation.commit,
        "package_name": expectation.package_name,
        "version": expectation.version,
    }
    if expectation.tag is not None:
        expected["tag"] = expectation.tag
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


def _recorded_identity(record: dict[str, object]) -> release_candidate.ReleaseIdentity:
    """Parse the identity object while retaining candidate-specific diagnostics."""
    raw_identity = record.get("identity")
    if not isinstance(raw_identity, dict):
        raise CandidateError(CandidateIssue.IDENTITY_OBJECT)
    try:
        return release_identity_record.identity_from_mapping(raw_identity)
    except release_candidate.IdentityError as error:
        raise CandidateError(CandidateIssue.IDENTITY_INVALID, (error,)) from error


def _recorded_digest(record: dict[str, object]) -> str:
    """Require the identity record's digest to be lowercase SHA-256 text."""
    digest = record.get("sha256")
    if not isinstance(digest, str) or len(digest) != SHA256_DIGEST_LENGTH:
        raise CandidateError(CandidateIssue.DIGEST_MISSING)
    if any(character not in "0123456789abcdef" for character in digest):
        raise CandidateError(CandidateIssue.DIGEST_FORMAT)
    return digest


def _validated_digest(archive: pathlib.Path, expected_digest: str) -> str:
    """Hash the retained archive and require the recorded candidate digest."""
    observed_digest = release_digest_files.digest_of_file(archive)
    if not release_digest.constant_time_equal(observed_digest, expected_digest):
        raise CandidateError(
            CandidateIssue.DIGEST_MISMATCH, (observed_digest, expected_digest)
        )
    return observed_digest


def validate_candidate(
    identity_path: pathlib.Path,
    archive_directory: pathlib.Path,
    expectation: CandidateExpectation,
) -> ValidatedCandidate:
    """Validate identity, artifact selection, digest and packaged manifest.

    A publishing expectation requires the identity to record exactly the
    expected tag. A rehearsal expectation requires the identity to record no
    tag. Both modes bind the commit, package name, and version. A rejection in
    any focused validation helper below surfaces as ``CandidateError``, so a
    mismatched identity, archive, digest, or packaged manifest fails
    validation rather than returning a partially checked candidate.

    Parameters
    ----------
    identity_path : pathlib.Path
        The package identity record to read.
    archive_directory : pathlib.Path
        The directory holding the sole retained ``.crate`` archive.
    expectation : CandidateExpectation
        The workflow identity, with ``tag`` set to ``None`` for a rehearsal.

    Returns
    -------
    ValidatedCandidate
        The identity and exact archive path and digest after validation.

    Examples
    --------
    >>> validate_candidate.__name__
    'validate_candidate'
    """
    record = _read_json_mapping(identity_path, "package identity")
    identity = _recorded_identity(record)
    _identity_matches(identity, expectation)
    crate, archive = crates_io_candidate_archive.archive_and_crate(
        record, archive_directory
    )
    observed_digest = _validated_digest(archive, _recorded_digest(record))
    _ = crates_io_candidate_archive.read_packaged_manifest(
        archive, expectation.package_name, expectation.version
    )
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
    # Exactly one release mode is required, mirroring ``package_crate_cli``.
    # A caller that supplies neither, or both, reaches an argument error rather
    # than a check that silently accepts whichever identity it was handed.
    release_mode = check.add_mutually_exclusive_group(required=True)
    release_mode.add_argument(
        "--tag", help="the release tag a publishing candidate must record"
    )
    release_mode.add_argument(
        "--rehearsal",
        action="store_true",
        help="accept a candidate that records no release tag",
    )
    check.add_argument("--commit", required=True)
    check.add_argument("--package-name", required=True)
    check.add_argument("--version", required=True)
    check.add_argument("--output", required=True, type=pathlib.Path)
    metadata = commands.add_parser("metadata", help="derive Cargo registry metadata")
    metadata.add_argument("--archive", required=True, type=pathlib.Path)
    metadata.add_argument("--output", required=True, type=pathlib.Path)
    return parser


def _record_for_arguments(arguments: argparse.Namespace) -> dict[str, object]:
    """Compute the requested candidate record before the output boundary writes it."""
    if arguments.command == "check":
        candidate = validate_candidate(
            arguments.identity,
            arguments.archive_directory,
            CandidateExpectation(
                None if arguments.rehearsal else arguments.tag,
                arguments.commit,
                arguments.package_name,
                arguments.version,
            ),
        )
        return candidate.as_record()
    return crates_io_upload_metadata.package_metadata(arguments.archive)


def main(argv: list[str] | None = None) -> int:
    """Run candidate validation or metadata derivation and return its status."""
    try:
        arguments = build_parser().parse_args(argv)
    except SystemExit as error:
        return EXIT_OK if error.code == 0 else EXIT_USAGE
    try:
        record = _record_for_arguments(arguments)
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
