"""Name every artefact a producer cannot vouch for, and why.

A producer hashes the files it is about to hand on, and every refusal it can
make is a statement about one of those files: it is absent, it is a symlink, it
is not one of the package formats this platform owes, or two paths would publish
under one name. Keeping that vocabulary here lets the reader in
:mod:`record_producer_provenance_record` stay focused on reading bytes, and lets
the CLI map a family of refusals to one exit code without matching on messages.

Messages live in the exception classes rather than at the raise sites, so a
caller composes a typed refusal and never an ad-hoc string.

Kept apart from the record writer so each file stays under the repository's
400-line cap.

Run via ``.github/actions/upload-declared-assets``'s producers; behavioural
coverage lives in ``scripts/tests/test_record_producer_provenance_cli.py``.
"""

import typing as typ

if typ.TYPE_CHECKING:
    import pathlib

__all__ = [
    "DoublyPublishedNameError",
    "DuplicatePackageFormatError",
    "IrregularArtifactError",
    "MissingPackageFormatError",
    "ProvenanceRefusedError",
    "UnreadableArtifactError",
    "UnrecordedPlatformError",
    "UnsafePublishedNameError",
    "UnusableInputError",
]


class UnusableInputError(ValueError):
    """Report a claimed identity or artefact this producer cannot vouch for."""


class UnrecordedPlatformError(UnusableInputError):
    """Report a platform whose package formats this release cannot name.

    Examples
    --------
    >>> str(UnrecordedPlatformError("solaris"))
    "platform 'solaris' stages no recorded package format"
    """

    def __init__(self, platform: str) -> None:
        """Build the diagnosis naming the platform.

        Parameters
        ----------
        platform
            The platform the caller claimed.
        """
        super().__init__(f"platform {platform!r} stages no recorded package format")


class DoublyPublishedNameError(UnusableInputError):
    """Report two artefacts that would publish under one name."""

    def __init__(self, name: str) -> None:
        """Build the diagnosis naming the contested asset.

        Parameters
        ----------
        name
            The published name both artefacts claim.
        """
        super().__init__(f"two artifacts publish under {name!r}")


class UnsafePublishedNameError(UnusableInputError):
    """Report a name that is not one bare file name."""

    def __init__(self, name: str) -> None:
        """Build the diagnosis naming the refused name.

        Parameters
        ----------
        name
            The name the caller's path resolves to.
        """
        super().__init__(f"artifact name {name!r} is unsafe to publish")


class UnreadableArtifactError(UnusableInputError):
    """Report an artefact whose type or bytes could not be read."""

    def __init__(self, path: pathlib.Path, failure: OSError) -> None:
        """Build the diagnosis naming the artefact and the read failure.

        Parameters
        ----------
        path
            The artefact that could not be read.
        failure
            The operating-system error raised while reading it.
        """
        super().__init__(f"artifact {str(path)!r} is unreadable: {failure}")


class IrregularArtifactError(UnusableInputError):
    """Report a path that is a symlink, a directory, or a special file."""

    def __init__(self, path: pathlib.Path) -> None:
        """Build the diagnosis naming the irregular artefact.

        Parameters
        ----------
        path
            The path that is not a regular file.
        """
        super().__init__(f"artifact {str(path)!r} is not a regular file")


class MissingPackageFormatError(UnusableInputError):
    """Report a producer that recorded none of a format its platform owes."""

    def __init__(self, target: str, formats: list[str]) -> None:
        """Build the diagnosis naming the target and the absent formats.

        Parameters
        ----------
        target
            The configured target whose producer is incomplete.
        formats
            The package suffixes the producer recorded nothing for.
        """
        super().__init__(
            f"producer for target {target!r} recorded no {' or '.join(formats)} package"
        )


class DuplicatePackageFormatError(UnusableInputError):
    """Report a producer that recorded two packages of one format."""

    def __init__(self, target: str, package_format: str, count: int) -> None:
        """Build the diagnosis naming the target, format, and count.

        Parameters
        ----------
        target
            The configured target whose producer is ambiguous.
        package_format
            The package suffix recorded more than once.
        count
            How many packages carried that suffix.
        """
        super().__init__(
            f"producer for target {target!r} recorded {count} "
            f"{package_format} packages, not one"
        )


class ProvenanceRefusedError(UnusableInputError):
    """Report a record the publication policy would refuse to read back."""

    def __init__(self, reason: str) -> None:
        """Build the diagnosis from the policy module's own message.

        Parameters
        ----------
        reason
            The policy module's diagnosis of the malformed record.
        """
        super().__init__(reason)
