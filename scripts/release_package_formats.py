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

import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    from pathlib import Path

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


class MalformedTargetError(ConfigurationError):
    """Report a target table that does not name its platform."""


class UnknownPlatformError(ConfigurationError):
    """Report a configured platform this build stages no package format for."""


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


def match_format(staged: cabc.Iterable[Path], package_format: str) -> list[Path]:
    """Return the staged packages carrying ``package_format`` as their suffix.

    Returns
    -------
    list[Path]
        The staged paths whose suffix is ``package_format``, in input order.

    Examples
    --------
    >>> from pathlib import Path
    >>> staged = [Path("netsuke-1.x86_64.rpm"), Path("netsuke-1_amd64.deb")]
    >>> [path.name for path in match_format(staged, ".rpm")]
    ['netsuke-1.x86_64.rpm']
    >>> match_format(staged, ".msi")
    []
    """
    return [path for path in staged if path.suffix == package_format]


def require_format_count(
    matches: list[Path], *, platform: str, package_format: str, target_count: int
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
