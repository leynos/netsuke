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
    "target_platforms",
]

#: Name of the target mapping in the parsed staging configuration.
TARGETS_TABLE = "targets"

#: Suffix appended to every published archive name.
SIDECAR_SUFFIX = ".sha256"


@dataclasses.dataclass(frozen=True, slots=True)
class StagingTarget:
    """One configured staging target, reduced to the platform it builds for.

    Attributes
    ----------
    platform : str
        The platform value that selects this target's package formats.
    """

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


def target_platforms(target_config: object) -> dict[str, str]:
    """Return the platform each configured target serves.

    Payload selection holds every producer record to the platform its target
    is configured with, so it needs the target keys as well as the per-platform
    counts the declaration uses. Both views are derived here from one validated
    reading of the ``targets`` table, so they cannot disagree.

    Returns
    -------
    dict[str, str]
        Configured target key mapped to the platform it stages packages for.

    Notes
    -----
    A configuration with no target surfaces ``MissingTargetsError`` and a
    malformed one ``MalformedTargetError``; both come from :func:`_target_table`
    and :func:`_platform_of` rather than being raised here.

    Examples
    --------
    >>> target_platforms({"targets": {"linux": {"platform": "linux"}}})
    {'linux': 'linux'}
    >>> target_platforms({"targets": {}})
    Traceback (most recent call last):
        ...
    release_package_formats.MissingTargetsError: staging configuration defines no \
release targets
    """
    targets = _target_table(target_config)
    return {target: _platform_of(entry) for target, entry in sorted(targets.items())}


@dataclasses.dataclass(frozen=True, slots=True)
class StagingTargets:
    """The configured staging targets, in configuration order.

    Attributes
    ----------
    targets : tuple[StagingTarget, ...]
        One entry per configured target.
    """

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

        Parameters
        ----------
        targets : collections.abc.Iterable[StagingTarget]
            The decoded staging targets, in configuration order.

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


def platform_target_counts(targets: StagingTargets) -> dict[str, int]:
    """Count decoded staging targets by the platform each one builds for.

    Parameters
    ----------
    targets : StagingTargets
        The decoded staging targets to count.

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


def _target_table(target_config: object) -> cabc.Mapping[str, object]:
    """Return the configuration's non-empty ``targets`` mapping.

    Returns
    -------
    collections.abc.Mapping[str, object]
        Each configured target key and the table describing it.

    Raises
    ------
    MissingTargetsError
        If the configuration carries no target at all.

    Examples
    --------
    >>> sorted(_target_table({"targets": {"b": {}, "a": {}}}))
    ['a', 'b']
    """
    config = _mapping(target_config)
    targets = config.get(TARGETS_TABLE)
    if not isinstance(targets, dict) or not targets:
        raise MissingTargetsError
    return targets


def _mapping(value: object) -> cabc.Mapping[str, object]:
    """Return a string-keyed mapping or refuse the malformed config value."""
    if not isinstance(value, dict) or not all(isinstance(key, str) for key in value):
        raise MalformedTargetError(MalformedTargetKind.INVALID_MAPPING)
    return value


def _platform_of(entry: object) -> str:
    """Return one target table's string platform value."""
    platform = entry.get("platform") if isinstance(entry, dict) else None
    if not isinstance(platform, str):
        raise MalformedTargetError(MalformedTargetKind.MISSING_PLATFORM)
    return platform


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
