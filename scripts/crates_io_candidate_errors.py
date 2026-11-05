"""Classify failures while validating the retained crate candidate.

The candidate check spans JSON, TOML, tar and filesystem boundaries, but every
failure it reports is a rejection of the same artifact. Keeping the issue
vocabulary here lets the readers that raise it stay focused on one boundary
each, without a reader importing the command-line module that composes them.
"""

import enum
import typing as typ


class CandidateIssue(enum.StrEnum):
    """Classify failures while checking the retained release candidate."""

    JSON_READ = "could not read {} {}: {}"
    JSON_OBJECT = "{} must be a JSON object"
    REHEARSAL = "a rehearsal identity cannot publish to crates.io"
    UNEXPECTED_TAG = "a rehearsal candidate must not record a release tag: {!r}"
    IDENTITY_MISMATCH = "candidate identity mismatch: {}"
    CRATE_BASENAME = "identity record crate must be a flat archive basename"
    CRATE_SUFFIX = "identity record crate {!r} must end in .crate"
    ARCHIVE_COUNT = "expected exactly one matching .crate named {!r}, found {!r}"
    ARCHIVE_FILE = "candidate archive is not a regular file: {!r}"
    MANIFEST_MEMBER = "archive must contain exactly one packaged Cargo.toml, found {!r}"
    MANIFEST_ROOT = "packaged manifest is rooted at {!r}, expected {!r}"
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
    """Carry a classified candidate error and its display values.

    Parameters
    ----------
    issue : CandidateIssue
        Candidate issue template to render.
    context : tuple[object, ...], default=()
        Values passed to the issue template.

    Examples
    --------
    >>> str(CandidateError(CandidateIssue.PACKAGE_TABLE))
    'packaged manifest has no [package] table'
    >>> str(CandidateError(CandidateIssue.UNEXPECTED_TAG, ("v1.2.3",)))
    "a rehearsal candidate must not record a release tag: 'v1.2.3'"
    """

    def __init__(self, issue: CandidateIssue, context: tuple[object, ...] = ()) -> None:
        """Store the candidate issue and values used to render its diagnostic."""
        self.issue = issue
        self.context = context
        super().__init__(str(self))

    @typ.override
    def __str__(self) -> str:
        """Render the issue with its associated diagnostic values."""
        return self.issue.value.format(*self.context)
