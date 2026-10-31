"""Declare the release assets from in-memory names and target facts.

This module owns release naming and package-count policy. The declaration CLI
reads TOML, decodes the staging targets into a :class:`StagingTargets` record,
asks the hoist adapter for archive names, and scans the staged tree; it passes
those facts here as a typed record and file names. Keeping paths, config
parsing, and the TOML field names at that boundary makes the declaration
independently testable.

Three external shared actions own the names of the operating-system packages,
so their names are observed from the tree rather than reconstructed. The
declaration CLI applies GitHub's asset-name normalization at that boundary;
this module holds each platform to the configured count and known formats.

Run via ``scripts/release_declaration_cli.py``; behavioural coverage lives in
``scripts/tests/test_release_declaration.py``.
"""

import dataclasses
import typing as typ

from release_package_formats import (
    PLATFORM_FORMATS,
    ConfigurationError,
    DeclarationError,
    DuplicatePackageError,
    MalformedTargetError,
    MalformedTargetKind,
    MissingPackageError,
    MissingTargetsError,
    StagedTreeError,
    UnknownPlatformError,
    match_format,
    require_format_count,
    require_unique_names,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc

__all__ = [
    "PLATFORM_FORMATS",
    "ConfigurationError",
    "DeclarationError",
    "DuplicatePackageError",
    "MalformedTargetError",
    "MissingPackageError",
    "MissingTargetsError",
    "StagedTreeError",
    "StagingTarget",
    "StagingTargets",
    "UnknownPlatformError",
    "archive_asset_names",
    "declared_asset_names",
    "platform_target_counts",
]

#: Suffix appended to every published archive name.
SIDECAR_SUFFIX = ".sha256"


@dataclasses.dataclass(frozen=True, slots=True)
class StagingTarget:
    """One configured staging target, reduced to the platform it builds for."""

    #: The platform value that selects this target's package formats.
    platform: str

    def __post_init__(self) -> None:
        """Refuse a target that names no platform string.

        The declaration counts platforms and sorts them, so a non-string
        platform reaches ``sorted`` as a mixed-type key set and fails with
        ``TypeError`` rather than a configuration diagnosis. An unknown
        platform *name* stays a policy refusal, raised by
        :func:`declared_asset_names`.

        Raises
        ------
        MalformedTargetError
            If the platform is not a non-empty string.

        Examples
        --------
        >>> StagingTarget("linux").platform
        'linux'
        >>> StagingTarget("")  # doctest: +ELLIPSIS
        Traceback (most recent call last):
            ...
        release_package_formats.MalformedTargetError: staging configuration has...
        """
        if not isinstance(self.platform, str) or not self.platform:
            raise MalformedTargetError(MalformedTargetKind.MISSING_PLATFORM)


@dataclasses.dataclass(frozen=True, slots=True)
class StagingTargets:
    """The configured staging targets, in configuration order."""

    #: One entry per configured target.
    targets: tuple[StagingTarget, ...]

    def __post_init__(self) -> None:
        """Refuse a record that names no target.

        Emptiness is refused here rather than in the factory, because
        :func:`declared_asset_names` accepts any record. A directly built
        empty record would declare archives and sidecars only, which is the
        smaller release this check exists to prevent.

        Raises
        ------
        MissingTargetsError
            If the record carries no target.

        Examples
        --------
        >>> StagingTargets((StagingTarget("linux"),)).targets
        (StagingTarget(platform='linux'),)
        >>> StagingTargets(())  # doctest: +ELLIPSIS
        Traceback (most recent call last):
            ...
        release_package_formats.MissingTargetsError: staging configuration defines...
        """
        if not self.targets:
            raise MissingTargetsError

    @classmethod
    def from_targets(cls, targets: cabc.Iterable[StagingTarget]) -> StagingTargets:
        """Build the record from a decoded configuration.

        Returns
        -------
        StagingTargets
            The decoded staging targets.

        Notes
        -----
        The non-empty invariant belongs to :meth:`__post_init__`, so every
        construction path refuses an empty configuration.

        Examples
        --------
        >>> StagingTargets.from_targets([StagingTarget("linux")]).targets
        (StagingTarget(platform='linux'),)
        >>> StagingTargets.from_targets([])  # doctest: +ELLIPSIS
        Traceback (most recent call last):
            ...
        release_package_formats.MissingTargetsError: staging configuration defines...
        """
        return cls(tuple(targets))


def archive_asset_names(archives: cabc.Iterable[str]) -> tuple[str, ...]:
    """Pair each configured archive name with its checksum-sidecar name.

    Returns
    -------
    tuple[str, ...]
        Each archive followed by the sidecar name that describes it.

    Examples
    --------
    >>> archive_asset_names(["netsuke.tar.gz"])
    ('netsuke.tar.gz', 'netsuke.tar.gz.sha256')
    """
    return tuple(
        name for archive in archives for name in (archive, f"{archive}{SIDECAR_SUFFIX}")
    )


def platform_target_counts(targets: StagingTargets) -> dict[str, int]:
    """Count decoded staging targets by the platform each one builds for.

    Returns
    -------
    dict[str, int]
        The number of targets for each platform.

    Examples
    --------
    >>> platform_target_counts(
    ...     StagingTargets((StagingTarget("linux"), StagingTarget("linux")))
    ... )
    {'linux': 2}
    """
    counts: dict[str, int] = {}
    for target in targets.targets:
        counts[target.platform] = counts.get(target.platform, 0) + 1
    return counts


def _packages_for_platform(
    staged_names: tuple[str, ...], platform: str, target_count: int
) -> list[str]:
    """Return a platform's published names after checking every format count."""
    names: list[str] = []
    for package_format in PLATFORM_FORMATS[platform]:
        matches = match_format(staged_names, package_format)
        require_format_count(
            matches,
            platform=platform,
            package_format=package_format,
            target_count=target_count,
        )
        names.extend(matches)
    require_unique_names(names)
    return sorted(names)


def _declared_packages(
    staged_names: tuple[str, ...], counts: cabc.Mapping[str, int]
) -> list[str]:
    """Return observed package names for every configured platform."""
    packages: list[str] = []
    for platform, target_count in sorted(counts.items()):
        if platform not in PLATFORM_FORMATS:
            raise UnknownPlatformError(platform)
        packages.extend(_packages_for_platform(staged_names, platform, target_count))
    return packages


def declared_asset_names(
    *,
    archive_names: cabc.Iterable[str],
    targets: StagingTargets,
    staged_package_names: cabc.Iterable[str],
) -> tuple[str, ...]:
    """Declare the flat consumer set from archive and package observations.

    Parameters
    ----------
    archive_names
        Archive names resolved by the hoist adapter from the manifest and
        staging configuration.
    targets
        Decoded staging targets, used to count package outputs by platform.
    staged_package_names
        GitHub-visible basenames observed below the downloaded artifact tree.

    Returns
    -------
    tuple[str, ...]
        Sorted archive names, sidecars, and observed operating-system packages.

    Examples
    --------
    >>> declared_asset_names(
    ...     archive_names=["crate.tar.gz"],
    ...     targets=StagingTargets((StagingTarget("linux"),)),
    ...     staged_package_names=["crate.deb", "crate.rpm"],
    ... )
    ('crate.deb', 'crate.rpm', 'crate.tar.gz', 'crate.tar.gz.sha256')
    >>> declared_asset_names(
    ...     archive_names=["crate.tar.gz"],
    ...     targets=StagingTargets((StagingTarget("linux"),)),
    ...     staged_package_names=[],
    ... )
    Traceback (most recent call last):
        ...
    release_package_formats.MissingPackageError: platform 'linux' ...
    """
    counts = platform_target_counts(targets)
    staged_names = tuple(staged_package_names)
    packages = _declared_packages(staged_names, counts)
    return tuple(sorted([*archive_asset_names(archive_names), *packages]))
