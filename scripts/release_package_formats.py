"""Determine which operating-system packages a platform's targets must stage.

Three external shared actions own the naming of the ``.deb``, ``.rpm``,
``.pkg`` and ``.msi`` files this repository publishes, so their names cannot be
derived from configuration the way the archives' can. What *is* knowable is how
many of each format a platform's targets must produce, and that count is what
:mod:`release_declaration` holds the staged tree to: a package that failed to
build becomes a refusal rather than a smaller declaration.

Holding the staged tree to a count only works if the count cannot silently
shrink, so the format list per platform is exhaustive and stated here once.

This module owns that policy and the refusals that enforce it. Assembling the
final declaration -- archives, sidecars and packages together -- belongs to
:mod:`release_declaration`, which knows what a release publishes but not what a
platform stages.

Run via ``scripts/release_declaration.py``; behavioural coverage lives in
``scripts/tests/test_release_declaration.py``.
"""

import enum
import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: The package formats the build stages, keyed by the ``platform`` value the
#: staging configuration names. The list per platform is exhaustive: a platform
#: absent from this mapping cannot be packaged at all, and one whose targets
#: produce anything else is refused rather than silently under-declared.
PLATFORM_FORMATS: dict[str, tuple[str, ...]] = {
    "linux": (".deb", ".rpm"),
    "macos": (".pkg",),
    "windows": (".msi",),
}


class DeclarationError(ValueError):
    """Report that the release cannot be declared from the given inputs."""


class ConfigurationError(DeclarationError):
    """Report a configuration the build cannot declare a release from.

    This is the input-side half of :class:`DeclarationError`: the file was read,
    but what it says cannot describe this release. An operator who sees this
    reads the staging configuration, not the build jobs.
    """


class MissingTargetsError(ConfigurationError):
    """Report a staging configuration that declares no release target."""

    def __init__(self) -> None:
        """Build the stable diagnosis for an empty release-target mapping.

        Examples
        --------
        >>> str(MissingTargetsError())
        'staging configuration defines no release targets'
        """
        super().__init__("staging configuration defines no release targets")


class MalformedTargetKind(enum.Enum):
    """Name one malformed target condition understood by declaration policy."""

    #: The staging configuration is not a string-keyed mapping.
    INVALID_MAPPING = "invalid-mapping"

    #: A target table has no string-valued platform field.
    MISSING_PLATFORM = "missing-platform"


class MalformedTargetError(ConfigurationError):
    """Report a target table that is not usable by release declaration."""

    def __init__(self, kind: MalformedTargetKind) -> None:
        """Build a target diagnosis from its typed validation condition.

        Examples
        --------
        >>> str(MalformedTargetError(MalformedTargetKind.MISSING_PLATFORM))
        'staging configuration has a target without a platform'
        >>> str(MalformedTargetError(MalformedTargetKind.INVALID_MAPPING))
        'staging configuration is not a string-keyed mapping'
        """
        messages = {
            MalformedTargetKind.INVALID_MAPPING: (
                "staging configuration is not a string-keyed mapping"
            ),
            MalformedTargetKind.MISSING_PLATFORM: (
                "staging configuration has a target without a platform"
            ),
        }
        super().__init__(messages[kind])


class UnknownPlatformError(ConfigurationError):
    """Report a configured platform this build stages no package format for."""

    def __init__(self, platform: str) -> None:
        """Build the diagnosis for a configured but unsupported platform.

        Examples
        --------
        >>> str(UnknownPlatformError("solaris"))
        "staging configuration declares unpackageable platform 'solaris'"
        """
        super().__init__(
            f"staging configuration declares unpackageable platform {platform!r}"
        )


class StagedTreeError(DeclarationError):
    """Report a staged tree that disagrees with the release the build promised.

    The build-side half: the configuration was understood, and the artefacts
    that arrived do not match it. An operator who sees this reads the job that
    was supposed to produce the missing asset.
    """


class MissingPackageError(StagedTreeError):
    """Report that a platform's staged packages disagree with its formats."""


class DuplicatePackageError(StagedTreeError):
    """Report that two staged packages claim one published name."""


def match_format(staged: cabc.Iterable[str], package_format: str) -> list[str]:
    """Return staged package names carrying ``package_format`` as a suffix.

    Returns
    -------
    list[str]
        The staged names whose suffix is ``package_format``, in input order.

    Examples
    --------
    >>> staged = ["netsuke-1.x86_64.rpm", "netsuke-1_amd64.deb"]
    >>> match_format(staged, ".rpm")
    ['netsuke-1.x86_64.rpm']
    >>> match_format(staged, ".msi")
    []
    """
    return [name for name in staged if name.endswith(package_format)]


def require_format_count(
    matches: list[str], *, platform: str, package_format: str, target_count: int
) -> None:
    """Refuse a format this platform staged the wrong number of files for.

    Raises
    ------
    MissingPackageError
        If ``matches`` does not hold exactly ``target_count`` entries.
    """
    if len(matches) == target_count:
        return
    msg = (
        f"platform {platform!r} must stage {target_count} "
        f"{package_format} package(s), found {len(matches)}"
    )
    raise MissingPackageError(msg)


def require_unique_names(names: cabc.Iterable[str]) -> None:
    """Refuse a set of names in which two staged files publish identically.

    The declaration would then name one asset twice and the upload step could
    publish either file; refusing is the only honest answer about which.

    Raises
    ------
    DuplicatePackageError
        If any name occurs more than once.
    """
    published = list(names)
    duplicated = sorted({name for name in published if published.count(name) > 1})
    if not duplicated:
        return
    msg = "staged packages publish under duplicated names: " + ", ".join(
        repr(name) for name in duplicated
    )
    raise DuplicatePackageError(msg)
