"""Resolve and record the one release candidate a publication run may publish.

Every byte a release uploads is derived from one peeled commit and validated
against one Cargo package identity. This module owns that identity: it holds
the tag/version agreement, refuses a near-match rather than normalizing one,
and serializes the result as a record later stages compare against. It performs
no filesystem, network, or process access, so its behaviour is fully decided by
its arguments.

The agreement is byte equality, deliberately. A validator that parsed both
sides into versions and compared those would accept `0.1.0-beta4+meta` for the
tag `v0.1.0-beta4`, because semantic-version ordering ignores build metadata.
That is exactly the near-match this release path must refuse: the tag names the
commit, and a version carrying metadata the tag does not spell is a different
package identity from the one a reviewer approved.

This module resolves an identity from facts just gathered. Reading a record
back from disk — which may have been hand-edited or left by an earlier run —
belongs to :mod:`release_identity_record`, because that input needs checking
this module's callers never need to do.
"""

import dataclasses
import json
import re

from release_semver import is_prerelease, is_valid_semver

#: A Cargo package-name-shaped identifier: ASCII alphanumerics, `-`, and `_`.
#: Cargo permits a leading digit and an underscore, so this refuses only what
#: could not appear on the registry at all — spaces, dots, path separators,
#: and non-ASCII text — rather than inventing a rule Cargo does not have.
_PACKAGE_NAME = re.compile(r"\A[A-Za-z0-9][A-Za-z0-9_-]*\Z")

#: A binary name is a file stem, so it takes the same shape as a package name.
_BIN_NAME = _PACKAGE_NAME

_FULL_COMMIT_SHA = re.compile(r"\A[0-9a-f]{40}\Z")


class IdentityError(ValueError):
    """Report candidate facts that cannot describe one publishable release.

    The subclasses below each name a distinct diagnosis, so a caller can
    decide what to do — report a stale tag, or refuse a malformed argument —
    without parsing an error message.
    """


class MalformedTagError(IdentityError):
    """Report a tag that is not `v` followed by a well-formed semantic version."""

    def __init__(self, tag: str) -> None:
        """Initialise the error naming the offending tag.

        Parameters
        ----------
        tag
            The tag as it was read from the release event.
        """
        super().__init__(
            f"tag {tag!r} is not a release tag: expected 'v' followed by a "
            f"semantic version such as 'v1.2.3'"
        )


class TagVersionMismatchError(IdentityError):
    """Report a tag whose spelling is not exactly the package version."""

    def __init__(self, tag: str, version: str) -> None:
        """Initialise the error naming both readings that disagreed.

        Parameters
        ----------
        tag
            The release tag.
        version
            The version declared by the manifest, which the tag must spell.
        """
        super().__init__(
            f"tag {tag!r} and package version {version!r} disagree; "
            f"the tag must be 'v{version}'"
        )


class MalformedCommitError(IdentityError):
    """Report a commit that is not a full lowercase 40-character SHA."""

    def __init__(self, commit: str) -> None:
        """Initialise the error naming the offending revision.

        Parameters
        ----------
        commit
            The revision the release event reported.
        """
        super().__init__(
            f"candidate commit {commit!r} must be a full lowercase "
            f"40-character SHA, not an abbreviated or symbolic revision"
        )


class MalformedNameError(IdentityError):
    """Report a package or binary name that could not appear in a manifest."""

    def __init__(self, label: str, value: str) -> None:
        """Initialise the error naming the field and the offending value.

        Parameters
        ----------
        label
            Which name was refused, such as ``"package name"``.
        value
            The refused value.
        """
        super().__init__(
            f"{label} {value!r} is not a valid Cargo identifier; expected "
            f"letters, digits, '-', and '_'"
        )


def _require_tag_version_agreement(tag: str | None, version: str) -> None:
    """Refuse any tag that does not spell the package version exactly.

    A ``None`` tag means no tag was supplied, which is what a rehearsal run
    looks like: a pull request has a merge ref rather than a release tag, so
    there is no tag whose spelling could agree with anything. An absent tag is
    therefore not a near-match to refuse but the recorded absence of one, and
    the resulting identity cannot name a publication candidate because it
    carries no tag to publish under.

    Raises
    ------
    MalformedTagError
        If the tag is not `v` followed by a well-formed semantic version.
    TagVersionMismatchError
        If the tag's spelling differs from ``version`` in any byte.

    Examples
    --------
    >>> _require_tag_version_agreement("v1.2.3", "1.2.3")
    >>> _require_tag_version_agreement(None, "1.2.3")
    """
    if tag is None:
        return
    if not tag.startswith("v") or not is_valid_semver(tag[1:]):
        raise MalformedTagError(tag)
    if tag[1:] != version:
        raise TagVersionMismatchError(tag, version)


def _require_commit(commit: str) -> None:
    """Refuse anything but a full lowercase 40-character commit SHA.

    Raises
    ------
    MalformedCommitError
        If ``commit`` is abbreviated, symbolic, or not lowercase hexadecimal.
    """
    if not _FULL_COMMIT_SHA.match(commit):
        raise MalformedCommitError(commit)


def _require_named(pattern: re.Pattern[str], value: str, label: str) -> None:
    """Refuse a value that cannot name a Cargo package or binary.

    Raises
    ------
    MalformedNameError
        If ``value`` does not match ``pattern``.
    """
    if not pattern.match(value):
        raise MalformedNameError(label, value)


@dataclasses.dataclass(frozen=True, slots=True)
class CandidateFacts:
    """Describe the facts gathered from one release event and manifest.

    Attributes
    ----------
    tag
        The release tag, such as ``"v0.1.0-beta4"``, or ``None`` when the run
        is a rehearsal with no tag to name.
    commit
        The full commit SHA the tag peels to.
    package_name
        The Cargo package name, such as ``"netsuke-build"``.
    version
        The version declared by the manifest, which the tag must spell.
    bin_name
        The installed binary name, such as ``"netsuke"``.
    """

    tag: str | None
    commit: str
    package_name: str
    version: str
    bin_name: str


@dataclasses.dataclass(frozen=True, slots=True)
class ReleaseIdentity:
    """Record the single candidate a publication run is authorised to publish.

    An instance exists only if :func:`resolve_identity` accepted its facts, so
    holding one is evidence that the tag, version, and commit agree. ``tag`` is
    ``None`` for a rehearsal identity, which records that no tag was supplied
    rather than inventing one.
    """

    tag: str | None
    commit: str
    package_name: str
    version: str
    bin_name: str
    is_prerelease: bool

    def as_record(self) -> dict[str, object]:
        """Return the identity as a JSON-serializable mapping.

        Keys are returned sorted, so two records diff only by content and a
        rendered record is stable across runs.

        Returns
        -------
        dict
            The recorded candidate facts, with sorted keys.

        Examples
        --------
        >>> identity = ReleaseIdentity("v1.2.3", "0" * 40, "p", "1.2.3", "b", False)
        >>> list(identity.as_record()) == sorted(identity.as_record())
        True
        """
        return {
            "bin_name": self.bin_name,
            "commit": self.commit,
            "is_prerelease": self.is_prerelease,
            "package_name": self.package_name,
            "tag": self.tag,
            "version": self.version,
        }

    def to_json(self) -> str:
        """Render the identity record as deterministic JSON.

        Returns
        -------
        str
            The record as indented JSON with sorted keys and a trailing
            newline, so a committed record is diff-stable.
        """
        return json.dumps(self.as_record(), sort_keys=True, indent=2) + "\n"


def resolve_identity(facts: CandidateFacts) -> ReleaseIdentity:
    """Validate candidate facts and record the identity they describe.

    Parameters
    ----------
    facts
        The tag, peeled commit, package name, version, and binary name read
        from one release event and its manifest.

    Returns
    -------
    ReleaseIdentity
        The validated identity, including the prerelease flag derived from the
        version rather than from the tag's spelling.

    Notes
    -----
    Failures surface from the validators this function delegates to rather
    than being raised here: ``MalformedCommitError`` or ``MalformedNameError``
    when a fact is malformed, and ``MalformedTagError`` or
    ``TagVersionMismatchError`` when the tag is not a release tag or does not
    spell the version. All four are ``IdentityError`` subclasses, so a caller
    that does not need to separate the diagnoses can catch the base type.

    Examples
    --------
    >>> facts = CandidateFacts(
    ...     tag="v1.2.3",
    ...     commit="0" * 40,
    ...     package_name="netsuke-build",
    ...     version="1.2.3",
    ...     bin_name="netsuke",
    ... )
    >>> resolve_identity(facts).is_prerelease
    False
    """
    _require_commit(facts.commit)
    _require_named(_PACKAGE_NAME, facts.package_name, "package name")
    _require_named(_BIN_NAME, facts.bin_name, "binary name")
    _require_tag_version_agreement(facts.tag, facts.version)
    return ReleaseIdentity(
        tag=facts.tag,
        commit=facts.commit,
        package_name=facts.package_name,
        version=facts.version,
        bin_name=facts.bin_name,
        is_prerelease=is_prerelease(facts.version),
    )
