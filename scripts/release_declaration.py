"""Declare the release assets from in-memory names and target facts.

This module owns release naming and package-count policy. The declaration CLI
reads TOML, asks the hoist adapter for archive names, and scans the staged tree;
it passes those facts here as mappings and file names. Keeping paths and config
parsing at that boundary makes the declaration independently testable.

Three external shared actions own the names of the operating-system packages,
so their names are observed from the tree rather than reconstructed. The
declaration CLI applies GitHub's asset-name normalization at that boundary;
this module holds each platform to the configured count and known formats.

Run via ``scripts/release_declaration_cli.py``; behavioural coverage lives in
``scripts/tests/test_release_declaration.py``.
"""

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
    "UnknownPlatformError",
    "archive_asset_names",
    "declared_asset_names",
    "platform_target_counts",
]

#: Name of the target mapping in the parsed staging configuration.
TARGETS_TABLE = "targets"

#: Suffix appended to every published archive name.
SIDECAR_SUFFIX = ".sha256"


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


def platform_target_counts(target_config: object) -> dict[str, int]:
    """Count configured targets by platform from an already parsed mapping.

    Returns
    -------
    dict[str, int]
        The number of configured targets for each platform.

    Raises
    ------
    MissingTargetsError
        If the ``targets`` mapping is absent or empty.

    Examples
    --------
    >>> platform_target_counts({"targets": {"linux": {"platform": "linux"}}})
    {'linux': 1}
    """
    config = _mapping(target_config)
    targets = config.get(TARGETS_TABLE)
    if not isinstance(targets, dict) or not targets:
        raise MissingTargetsError
    counts: dict[str, int] = {}
    for entry in targets.values():
        platform = _platform_of(entry)
        counts[platform] = counts.get(platform, 0) + 1
    return counts


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
    target_config: object,
    staged_package_names: cabc.Iterable[str],
) -> tuple[str, ...]:
    """Declare the flat consumer set from archive and package observations.

    Parameters
    ----------
    archive_names
        Archive names resolved by the hoist adapter from the manifest and
        staging configuration.
    target_config
        Parsed staging TOML used to count package outputs by platform.
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
    ...     target_config={"targets": {"linux": {"platform": "linux"}}},
    ...     staged_package_names=["crate.deb", "crate.rpm"],
    ... )
    ('crate.deb', 'crate.rpm', 'crate.tar.gz', 'crate.tar.gz.sha256')
    >>> declared_asset_names(
    ...     archive_names=["crate.tar.gz"],
    ...     target_config={"targets": {"linux": {"platform": "linux"}}},
    ...     staged_package_names=[],
    ... )
    Traceback (most recent call last):
        ...
    release_package_formats.MissingPackageError: platform 'linux' ...
    """
    counts = platform_target_counts(target_config)
    staged_names = tuple(staged_package_names)
    packages = _declared_packages(staged_names, counts)
    return tuple(sorted([*archive_asset_names(archive_names), *packages]))
