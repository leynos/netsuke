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
    "target_platforms",
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


def platform_target_counts(target_config: object) -> dict[str, int]:
    """Count configured targets by platform from an already parsed mapping.

    Returns
    -------
    dict[str, int]
        The number of configured targets for each platform.

    Notes
    -----
    An unusable target table surfaces the same diagnoses as
    :func:`target_platforms`, because this counts the view that function
    returns.

    Examples
    --------
    >>> platform_target_counts({"targets": {"linux": {"platform": "linux"}}})
    {'linux': 1}
    """
    counts: dict[str, int] = {}
    for platform in target_platforms(target_config).values():
        counts[platform] = counts.get(platform, 0) + 1
    return counts


def _mapping(value: object) -> cabc.Mapping[str, object]:
    """Return a string-keyed mapping or refuse the malformed config value."""
    if not isinstance(value, dict) or not all(isinstance(key, str) for key in value):
        raise MalformedTargetError(MalformedTargetKind.INVALID_MAPPING)
    return value


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
