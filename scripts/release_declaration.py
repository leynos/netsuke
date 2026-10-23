"""Declare exactly the asset names a correct release publishes.

The release publishes a flat consumer set: the six binstall archives, their six
``.sha256`` sidecars, and the eight operating-system packages. This module is
the single place that set is stated, so the upload step, the inventory verifier
and the finalizer cannot each invent their own answer.

The declaration has two sources on purpose, and neither can shrink silently.
Archive and sidecar names are derived from configuration — the staging
configuration's target triples and the manifest's package name — through
:func:`release_inventory.binstall_asset_names`, which in turn delegates to the
hoist's own authority. The operating-system packages are read from the staged
tree, because three external shared actions own their naming and reimplementing
those rules here would be an unverifiable guess; the tree is therefore held to
a per-platform format count, so a package that failed to build is a refusal
rather than a smaller declaration.

Everything else the build stages — per-platform binaries, completions, manual
pages, PowerShell help, ``LICENSE`` — ships *inside* those packages and is not
a release asset. That exclusion is what keeps the declaration correct: a GitHub
release is a flat namespace holding six distinct binaries called ``netsuke``,
and any rule broad enough to admit one of them admits twelve files under a name
another file already claims.

One normalisation is applied to the package names. GitHub rewrites special and
non-alphanumeric characters in an asset name, and over the names this
repository produces the only such character is the ``~`` a prerelease version
puts in the four ``.deb``/``.rpm`` names; GitHub republishes each with ``.`` in
its place. The mapping is applied here so the declaration names what a consumer
downloads, rather than the spelling the tree happens to hold. It is
deliberately not a general sanitiser: predicting a name no upload produces is
as wrong as predicting the staged one.

Declarations are what makes a release checkable at all. On 2026-10-12 every
published release carried its six archives' ``.sha256`` sidecars and none of
the archives, because the uploader's own naming rule reduced 130 staged files
to 29 published names and the archives were not among them. A declaration built
by modelling that rule would have certified the same broken release, so this
module declares what a *correct* upload publishes and leaves the comparison to
:func:`release_inventory.validate_inventory`.

Run via ``scripts/release_declaration_cli.py`` in the release workflow;
behavioural coverage lives in ``scripts/tests/test_release_declaration.py``.
"""

import tomllib
import typing as typ

import hoist_binstall_discovery
import release_inventory
import release_package_formats
from release_package_formats import (
    ConfigurationError,
    DeclarationError,
    DuplicatePackageError,
    MalformedTargetError,
    MissingPackageError,
    MissingTargetsError,
    PLATFORM_FORMATS,
    StagedTreeError,
    UnknownPlatformError,
    match_format,
    require_format_count,
    require_unique_names,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    from pathlib import Path

#: Re-exported so this module remains the single name a release caller imports
#: for the refusals and format policy. The definitions live in
#: :mod:`release_package_formats`, which owns them; the list is explicit so a
#: name removed there fails here rather than silently disappearing.
__all__ = [
    "ConfigurationError",
    "DeclarationError",
    "DuplicatePackageError",
    "MalformedTargetError",
    "MissingPackageError",
    "MissingTargetsError",
    "PLATFORM_FORMATS",
    "StagedTreeError",
    "UnknownPlatformError",
    "declared_asset_names",
    "platform_target_counts",
    "published_name",
    "write_declaration",
]

#: The table a configuration must carry for its targets to be declared at all.
#: Read here rather than by :func:`hoist_binstall_discovery.expected_archive_names`
#: because that function needs each entry's ``target`` triple while this module
#: needs only its ``platform``: a configuration carrying the former but not the
#: latter is a different defect and deserves its own diagnosis.
TARGETS_TABLE = "targets"

#: Characters GitHub rewrites in a published asset name, mapped to what it
#: publishes in their place. Only ``~`` occurs in a name this repository
#: produces: a prerelease version spells it between the version and the
#: package revision, and GitHub republishes the name with ``.`` there.
PUBLISHED_CHARACTERS = {"~": "."}


def published_name(staged_name: str) -> str:
    """Return the name GitHub publishes for a staged asset name.

    Parameters
    ----------
    staged_name
        Bare file name as the build staged it.

    Returns
    -------
    str
        The name with every character in :data:`PUBLISHED_CHARACTERS` mapped
        to what GitHub publishes in its place.

    Examples
    --------
    An RPM carrying a prerelease version keeps its bytes but not its
    spelling, while a package with no special character passes unchanged:

    >>> published_name("netsuke-0.1.0~beta4-1.x86_64.rpm")
    'netsuke-0.1.0.beta4-1.x86_64.rpm'
    >>> published_name("netsuke-0.1.0-beta4-macos-arm64.pkg")
    'netsuke-0.1.0-beta4-macos-arm64.pkg'
    """
    published = staged_name
    for staged, replacement in PUBLISHED_CHARACTERS.items():
        published = published.replace(staged, replacement)
    return published


def platform_target_counts(staging_config: Path) -> dict[str, int]:
    """Count the configured release targets per platform.

    Parameters
    ----------
    staging_config
        Path to ``.github/release-staging.toml``.

    Returns
    -------
    dict[str, int]
        How many targets each ``platform`` value declares.

    Raises
    ------
    MissingTargetsError
        If the configuration declares no target at all, so no release can be
        described from it.

    Notes
    -----
    A target table carrying no string ``platform`` raises
    ``MalformedTargetError`` from :func:`_platform_of`, which propagates
    unchanged.

    Only the ``platform`` field is read here. The target triples belong to the
    archive half, which :func:`hoist_binstall_discovery.expected_archive_names`
    already owns; reading them here would create a second authority for them.

    An unreadable configuration propagates ``OSError``, and a malformed one
    ``tomllib.TOMLDecodeError``. A configuration naming a platform this build
    stages no format for raises ``UnknownPlatformError`` from
    :func:`declared_asset_names`, not here: counting is not judging.
    """
    config = tomllib.loads(staging_config.read_text(encoding="utf-8"))
    targets = config.get(TARGETS_TABLE, {})
    if not targets:
        msg = f"{staging_config} defines no release targets"
        raise MissingTargetsError(msg)
    counts: dict[str, int] = {}
    for entry in targets.values():
        platform = _platform_of(staging_config, entry)
        counts[platform] = counts.get(platform, 0) + 1
    return counts


def _platform_of(staging_config: Path, entry: object) -> str:
    """Return one target table's ``platform`` value.

    Parameters
    ----------
    staging_config
        Path the target was read from, named in any error message.
    entry
        One value from the configuration's ``targets`` table.

    Returns
    -------
    str
        The platform the target packages for.

    Raises
    ------
    MalformedTargetError
        If the target table carries no string ``platform``. A target that does
        not name its platform cannot be counted, and counting it as a platform
        of its own would inflate the declaration instead of refusing it.
    """
    platform = entry.get("platform") if isinstance(entry, dict) else None
    if not isinstance(platform, str):
        msg = f"{staging_config} has a target without a string platform"
        raise MalformedTargetError(msg)
    return platform


def _packages_for_platform(
    dist_dir: Path, platform: str, target_count: int
) -> list[str]:
    """Return one platform's published package names, or explain the shortfall.

    Parameters
    ----------
    dist_dir
        Release staging root the workflow artefacts were downloaded into.
    platform
        A ``platform`` value from the staging configuration, already known to
        be one this build packages.
    target_count
        How many targets the platform declares.

    Returns
    -------
    list[str]
        Sorted published names, one per staged package file.

    Raises
    ------
    MissingPackageError
        If any format is represented by a number of files other than
        ``target_count``.
    DuplicatePackageError
        If two staged files, wherever they sit in the tree, publish under one
        name.

    Notes
    -----
    The count is checked per format before any name is mapped, so a shortfall
    is reported as the missing package rather than as whichever duplicate or
    surplus the mapping would have produced first.
    """
    staged = hoist_binstall_discovery.walk_files(dist_dir)
    names: list[str] = []
    for package_format in PLATFORM_FORMATS[platform]:
        matches = match_format(staged, package_format)
        require_format_count(
            matches,
            platform=platform,
            package_format=package_format,
            target_count=target_count,
        )
        names.extend(published_name(path.name) for path in matches)
    require_unique_names(names)
    return sorted(names)


def _declared_packages(
    dist_dir: Path, staging_config: Path, counts: cabc.Mapping[str, int]
) -> list[str]:
    """Return every staged package's published name, across all platforms.

    Parameters
    ----------
    dist_dir
        Release staging root the workflow artefacts were downloaded into.
    staging_config
        Path to the staging configuration, named in the refusal below.
    counts
        How many targets each platform declares.

    Returns
    -------
    list[str]
        One published name per staged package file, all platforms together.

    Raises
    ------
    UnknownPlatformError
        If the configuration declares a platform this build cannot package.
    MissingPackageError, DuplicatePackageError
        Propagated from :func:`_packages_for_platform`.
    """
    packages: list[str] = []
    for platform, target_count in sorted(counts.items()):
        if platform not in PLATFORM_FORMATS:
            msg = f"{staging_config} declares targets for unpackageable {platform!r}"
            raise UnknownPlatformError(msg)
        packages.extend(_packages_for_platform(dist_dir, platform, target_count))
    return packages


def declared_asset_names(
    dist_dir: Path,
    staging_config: Path,
    manifest: Path,
    version: str,
) -> tuple[str, ...]:
    """Declare the asset names a correct release publishes, sorted.

    Parameters
    ----------
    dist_dir
        Release staging root holding the downloaded workflow artefacts.
    staging_config
        Path to ``.github/release-staging.toml``.
    manifest
        Path to the Cargo manifest naming the package.
    version
        Release version interpolated into each archive name.

    Returns
    -------
    tuple[str, ...]
        The archives and their sidecars followed by the operating-system
        packages, sorted so the declaration is stable between runs.

    Raises
    ------
    UnknownPlatformError
        If the configuration declares a target for a platform this build has
        no package format for, so that platform's assets would be omitted.

    Notes
    -----
    The archive half is resolved before the package half, deliberately: it
    needs no staged file at all, so a tree holding no packages still declares
    the archives and then fails on the packages, rather than reporting a
    smaller release. Every :class:`DeclarationError` the helpers raise
    propagates unchanged; :func:`_declared_packages` and
    :func:`_packages_for_platform` state which each can produce.

    Examples
    --------
    An empty tree declares the archives and then refuses, because the
    operating-system packages it must also publish are absent. The
    configuration below declares one ``linux`` target, so two package files
    are required:

    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     root = Path(tmp)
    ...     staging = chr(10).join([
    ...         "[targets.a]",
    ...         'platform = "linux"',
    ...         'target = "x86_64-unknown-linux-gnu"',
    ...     ])
    ...     _ = (root / "s.toml").write_text(staging, encoding="utf-8")
    ...     manifest = chr(10).join(["[package]", 'name = "p"'])
    ...     _ = (root / "Cargo.toml").write_text(manifest, encoding="utf-8")
    ...     (root / "dist").mkdir()
    ...     declared_asset_names(root / "dist", root / "s.toml",
    ...                          root / "Cargo.toml", "1.0.0")
    Traceback (most recent call last):
        ...
    release_package_formats.MissingPackageError: platform 'linux' must stage \
1 .deb package(s), found 0
    """
    counts = platform_target_counts(staging_config)
    archives = release_inventory.binstall_asset_names(staging_config, manifest, version)
    packages = _declared_packages(dist_dir, staging_config, counts)
    return tuple(sorted([*archives, *packages]))


def write_declaration(names: cabc.Iterable[str], destination: Path) -> None:
    """Write the declaration as one asset name per line.

    Parameters
    ----------
    names
        Asset names to declare.
    destination
        File the declaration is written to.

    Notes
    -----
    The format is the one ``verify_release_inventory_cli --expected`` reads, so
    the producer and its consumer agree by construction rather than by two
    implementations of the same parsing rule. A trailing newline is written:
    the reader ignores blank lines, and a file an editor would flag as
    unterminated is a nuisance rather than a defect.
    """
    destination.write_text("".join(f"{name}\n" for name in names), encoding="utf-8")
