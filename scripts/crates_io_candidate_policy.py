"""Apply candidate acceptance rules to parsed identity and manifest facts.

The command-line module reads the identity record and asks the archive adapter
for the packaged manifest; this module owns the release-mode, identity, and
packaged-manifest comparisons those values are subject to. Every function
decides from in-memory values alone, so the rules are testable without a
filesystem or an archive, and neither reader has to enforce a decision it is
not responsible for.
"""

import dataclasses
import typing as typ

from crates_io_candidate_errors import CandidateError, CandidateIssue

if typ.TYPE_CHECKING:
    import release_candidate


@dataclasses.dataclass(frozen=True, slots=True)
class CandidateExpectation:
    """Carry the workflow identity a packaged candidate must match.

    ``tag`` is ``None`` for a rehearsal. A pull request has a merge ref rather
    than a release tag, so a rehearsed candidate records no tag and the check
    binds only the commit, package name, and version. A publishing run supplies
    the tag, and the identity must then record exactly that tag.

    Attributes
    ----------
    tag : str | None
        The release tag the candidate must record. ``None`` selects rehearsal
        mode.
    commit : str
        The commit the candidate must record.
    package_name : str
        The package the candidate must package.
    version : str
        The version the candidate must package.
    """

    tag: str | None
    commit: str
    package_name: str
    version: str

    @property
    def is_rehearsal(self) -> bool:
        """Whether this expectation accepts a tagless candidate."""
        return self.tag is None


def require_release_mode(
    identity: release_candidate.ReleaseIdentity,
    expectation: CandidateExpectation,
) -> None:
    """Require the recorded tag to agree with the run's release mode.

    Parameters
    ----------
    identity : release_candidate.ReleaseIdentity
        The identity the candidate artifact records.
    expectation : CandidateExpectation
        The workflow identity the candidate must match.

    Raises
    ------
    CandidateError
        If a rehearsal candidate records a tag, or a publishing candidate
        records none.

    Examples
    --------
    >>> import release_candidate
    >>> identity = release_candidate.ReleaseIdentity(
    ...     "v1.2.3", "0" * 40, "p", "1.2.3", "b", is_prerelease=False
    ... )
    >>> require_release_mode(
    ...     identity, CandidateExpectation("v1.2.3", "0" * 40, "p", "1.2.3")
    ... )
    """
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


def require_identity_matches(
    identity: release_candidate.ReleaseIdentity,
    expectation: CandidateExpectation,
) -> None:
    """Require the artifact identity to match this validated release run.

    The release mode is checked first so that a tagless rehearsal and a
    tag-carrying publication each report their own rejection. The remaining
    fields are compared together, and every disagreement is named in the
    diagnostic so a caller can see all of them at once.

    Parameters
    ----------
    identity : release_candidate.ReleaseIdentity
        The identity the candidate artifact records.
    expectation : CandidateExpectation
        The workflow identity the candidate must match.

    Raises
    ------
    CandidateError
        If the release mode disagrees, or any recorded identity fact differs
        from the expected workflow value.

    Examples
    --------
    >>> import release_candidate
    >>> identity = release_candidate.ReleaseIdentity(
    ...     "v1.2.3", "0" * 40, "p", "1.2.3", "b", is_prerelease=False
    ... )
    >>> require_identity_matches(
    ...     identity, CandidateExpectation("v1.2.3", "0" * 40, "p", "1.2.3")
    ... )
    """
    require_release_mode(identity, expectation)
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


def require_manifest_identity(
    root: str,
    manifest: dict[str, object],
    package_name: str,
    version: str,
) -> None:
    """Require the packaged manifest identity to match the validated workflow.

    Parameters
    ----------
    root : str
        The single directory segment the archive places ``Cargo.toml`` in.
    manifest : dict[str, object]
        The parsed packaged manifest.
    package_name : str
        The package the validated release must package.
    version : str
        The version the validated release must package.

    Raises
    ------
    CandidateError
        If the archive root is not ``<package>-<version>``, the package table
        names another package or version, or the manifest has no package table
        at all.

    Examples
    --------
    >>> require_manifest_identity(
    ...     "p-1.2.3", {"package": {"name": "p", "version": "1.2.3"}}, "p", "1.2.3"
    ... )
    """
    # A root that is not the package-version directory means the archive holds
    # someone else's manifest, even when that manifest declares the expected
    # identity; checking the root first keeps the more specific diagnosis.
    expected_root = f"{package_name}-{version}"
    if root != expected_root:
        raise CandidateError(CandidateIssue.MANIFEST_ROOT, (root, expected_root))
    # A missing or non-mapping package table is a structural fault, while a
    # mapping that names another package is an identity mismatch; the two
    # issues stay distinct so the workflow can report which one occurred.
    match manifest.get("package"):
        case {"name": name, "version": found} if (
            name == package_name and found == version
        ):
            return
        case dict():
            raise CandidateError(CandidateIssue.MANIFEST_IDENTITY)
        case _:
            raise CandidateError(CandidateIssue.PACKAGE_TABLE)
