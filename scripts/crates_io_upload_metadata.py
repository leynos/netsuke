"""Derive crates.io upload metadata from the retained Cargo archive.

This module reads the packaged manifest and README in memory and normalizes
Cargo dependency tables to the registry web API schema. It neither selects nor
mutates candidate archives; :mod:`crates_io_candidate_cli` validates those.
"""

import dataclasses
import enum
import io
import pathlib
import tarfile
import tomllib


class MetadataIssue(enum.StrEnum):
    """Classify malformed metadata found in the retained crate archive."""

    STRING_ARRAY = "manifest field {!r} must be an array of strings"
    README_PATH = "unsafe packaged README path {!r}"
    README_ABSENT = "packaged README {!r} is absent"
    README_READ = "could not read packaged README {!r}"
    DEPENDENCY_SPEC = "dependency {!r} has an invalid specification"
    DEPENDENCY_VERSION = "dependency {!r} has no publishable version"
    DEPENDENCY_FLAGS = "dependency {!r} feature flags must be booleans"
    DEPENDENCY_RENAME = "dependency {!r} package rename must be a string"
    DEPENDENCY_REGISTRY = "dependency {!r} registry must be a string or null"
    MANIFEST_TABLE = "manifest table {!r} must be an object"
    TARGET_TABLE = "manifest [target] table must be an object"
    TARGET_SHAPE = "target-specific dependency tables are malformed"
    TARGET_DEPENDENCIES = "target table {!r} must be an object"
    MANIFEST_COUNT = "archive must contain exactly one Cargo.toml"
    MANIFEST_READ = "could not read packaged Cargo.toml"
    PACKAGE_TABLE = "packaged manifest has no [package] table"
    ARCHIVE_READ = "could not inspect packaged crate metadata: {}"
    PACKAGE_IDENTITY = "packaged manifest name and version must be strings"
    PACKAGE_FEATURES = "manifest badges and features must be objects"


class MetadataError(ValueError):
    """Carry a classified metadata issue and its display values."""

    def __init__(self, issue: MetadataIssue, context: tuple[object, ...] = ()) -> None:
        """Store the metadata issue and values used to render its diagnostic."""
        self.issue = issue
        self.context = context
        super().__init__(str(self))

    def __str__(self) -> str:
        """Render the issue with its associated diagnostic values."""
        return self.issue.value.format(*self.context)


_DEPENDENCY_KINDS = (
    ("dependencies", "normal"),
    ("dev-dependencies", "dev"),
    ("build-dependencies", "build"),
)
_AUTO_READMES = ("README.md", "README.txt", "README")


@dataclasses.dataclass(frozen=True, slots=True)
class PackagedManifest:
    """Hold parsed package data and README bytes read from one crate archive."""

    manifest: dict[str, object]
    package: dict[str, object]
    readme: str | None
    readme_file: str | None


def _metadata_table(
    value: object, issue: MetadataIssue, context: tuple[object, ...] = ()
) -> dict[str, object]:
    """Require a manifest table with its boundary-specific diagnostic."""
    if not isinstance(value, dict):
        raise MetadataError(issue, context)
    return value


def _string_list(value: object, field: str) -> list[str]:
    """Return a string array from a manifest field, or an empty array."""
    if value is None:
        return []
    if not isinstance(value, list) or any(not isinstance(item, str) for item in value):
        raise MetadataError(MetadataIssue.STRING_ARRAY, (field,))
    return value


def _select_readme_member(
    package: tarfile.TarFile, root: str, candidates: list[str]
) -> str | None:
    """Select the first regular README member after validating each path."""
    for candidate in candidates:
        relative = pathlib.PurePosixPath(candidate)
        if not _is_safe_readme_path(candidate, relative):
            raise MetadataError(MetadataIssue.README_PATH, (candidate,))
        member_name = f"{root}/{relative.as_posix()}"
        member = next(
            (item for item in package.getmembers() if item.name == member_name), None
        )
        if member is not None and member.isfile():
            return member_name
    return None


def _readme_contents(package: tarfile.TarFile, selected: str) -> str:
    """Read the selected README member as UTF-8 without extracting it."""
    member_file = package.extractfile(selected)
    if member_file is None:
        raise MetadataError(MetadataIssue.README_READ, (selected,))
    return member_file.read().decode("utf-8")


def _readme_file(
    package: tarfile.TarFile, root: str, package_table: dict[str, object]
) -> tuple[str | None, str | None]:
    """Read packaged README contents and return the relative path."""
    configured = package_table.get("readme")
    if configured is False:
        return None, None
    candidates = [configured] if isinstance(configured, str) else list(_AUTO_READMES)
    selected = _select_readme_member(package, root, candidates)
    if selected is None:
        if isinstance(configured, str):
            raise MetadataError(MetadataIssue.README_ABSENT, (configured,))
        return None, None
    return _readme_contents(package, selected), selected.removeprefix(f"{root}/")


def _is_safe_readme_path(candidate: str, path: pathlib.PurePosixPath) -> bool:
    """Reject absolute, parent-relative or platform-ambiguous README paths."""
    return not path.is_absolute() and ".." not in path.parts and "\\" not in candidate


def _dependency_specification(name: str, specification: object) -> dict[str, object]:
    """Normalize a dependency's string or table specification to a mapping."""
    match specification:
        case str() as version:
            return {"version": version}
        case dict() as fields:
            return fields
        case _:
            raise MetadataError(MetadataIssue.DEPENDENCY_SPEC, (name,))


def _dependency_flags(name: str, fields: dict[str, object]) -> tuple[bool, bool]:
    """Require optional and default-feature flags to retain Cargo's booleans."""
    optional = fields.get("optional", False)
    default_features = fields.get("default-features", True)
    if not isinstance(optional, bool) or not isinstance(default_features, bool):
        raise MetadataError(MetadataIssue.DEPENDENCY_FLAGS, (name,))
    return optional, default_features


def _dependency(
    name: str, specification: object, kind: str, target: str | None
) -> dict[str, object]:
    """Normalize one Cargo dependency into registry upload fields."""
    fields = _dependency_specification(name, specification)
    version_req = fields.get("version")
    if not isinstance(version_req, str):
        raise MetadataError(MetadataIssue.DEPENDENCY_VERSION, (name,))
    features = _string_list(fields.get("features"), f"dependency {name}.features")
    optional, default_features = _dependency_flags(name, fields)
    package_name = fields.get("package", name)
    if not isinstance(package_name, str):
        raise MetadataError(MetadataIssue.DEPENDENCY_RENAME, (name,))
    registry = fields.get("registry")
    if registry is not None and not isinstance(registry, str):
        raise MetadataError(MetadataIssue.DEPENDENCY_REGISTRY, (name,))
    return {
        "default_features": default_features,
        "explicit_name_in_toml": name if package_name != name else None,
        "features": features,
        "kind": kind,
        "name": package_name,
        "optional": optional,
        "registry": registry,
        "target": target,
        "version_req": version_req,
    }


def _dependency_tables(
    tables: dict[str, object], target: str | None, issue: MetadataIssue
) -> list[tuple[str, dict[str, object], str | None]]:
    """Collect the three dependency kinds for one manifest or target table."""
    sources: list[tuple[str, dict[str, object], str | None]] = []
    for table_name, kind in _DEPENDENCY_KINDS:
        table = _metadata_table(tables.get(table_name, {}), issue, (table_name,))
        sources.append((kind, table, target))
    return sources


def _target_dependency_tables(
    manifest: dict[str, object],
) -> list[tuple[str, dict[str, object], str | None]]:
    """Collect validated dependency tables for each target in insertion order."""
    targets = _metadata_table(manifest.get("target", {}), MetadataIssue.TARGET_TABLE)
    sources: list[tuple[str, dict[str, object], str | None]] = []
    for target, raw_tables in targets.items():
        if not isinstance(target, str):
            raise MetadataError(MetadataIssue.TARGET_SHAPE)
        tables = _metadata_table(raw_tables, MetadataIssue.TARGET_SHAPE)
        sources.extend(
            _dependency_tables(tables, target, MetadataIssue.TARGET_DEPENDENCIES)
        )
    return sources


def _dependency_records(manifest: dict[str, object]) -> list[dict[str, object]]:
    """Flatten normal, development, build and target-specific dependencies."""
    sources = _dependency_tables(manifest, None, MetadataIssue.MANIFEST_TABLE)
    sources.extend(_target_dependency_tables(manifest))
    return [
        _dependency(name, specification, kind, target)
        for kind, table, target in sources
        for name, specification in sorted(table.items())
    ]


def _manifest_member(archive: tarfile.TarFile) -> tarfile.TarInfo:
    """Require the retained archive to contain one regular Cargo manifest."""
    manifests = [
        member for member in archive.getmembers() if member.name.endswith("/Cargo.toml")
    ]
    if len(manifests) != 1 or not manifests[0].isfile():
        raise MetadataError(MetadataIssue.MANIFEST_COUNT)
    return manifests[0]


def _manifest_tables(
    archive: tarfile.TarFile, member: tarfile.TarInfo
) -> tuple[dict[str, object], dict[str, object]]:
    """Parse the packaged manifest and require its package table."""
    manifest_file = archive.extractfile(member)
    if manifest_file is None:
        raise MetadataError(MetadataIssue.MANIFEST_READ)
    manifest = tomllib.loads(manifest_file.read().decode("utf-8"))
    package = _metadata_table(manifest.get("package"), MetadataIssue.PACKAGE_TABLE)
    return manifest, package


def _read_packaged_manifest(archive_path: pathlib.Path) -> PackagedManifest:
    """Read the sole packaged manifest and README without extracting files."""
    try:
        archive_bytes = archive_path.read_bytes()
        with tarfile.open(fileobj=io.BytesIO(archive_bytes), mode="r:gz") as archive:
            member = _manifest_member(archive)
            manifest, package = _manifest_tables(archive, member)
            root = member.name.removesuffix("/Cargo.toml")
            readme, readme_file = _readme_file(archive, root, package)
    except (OSError, UnicodeError, tarfile.TarError, tomllib.TOMLDecodeError) as error:
        raise MetadataError(MetadataIssue.ARCHIVE_READ, (error,)) from error
    return PackagedManifest(manifest, package, readme, readme_file)


def package_metadata(archive_path: pathlib.Path) -> dict[str, object]:
    """Derive the crates.io publish record from the archive's own manifest.

    Returns
    -------
    dict[str, object]
        A crates.io upload record derived from packaged Cargo metadata.

    Raises
    ------
    MetadataError
        If the archive metadata is malformed or cannot map to the registry
        publish schema.

    Examples
    --------
    >>> package_metadata.__name__
    'package_metadata'
    """
    packaged = _read_packaged_manifest(archive_path)
    manifest = packaged.manifest
    package_table = packaged.package
    name = package_table.get("name")
    version = package_table.get("version")
    if not isinstance(name, str) or not isinstance(version, str):
        raise MetadataError(MetadataIssue.PACKAGE_IDENTITY)
    badges = manifest.get("badges", {})
    features = manifest.get("features", {})
    badges = _metadata_table(badges, MetadataIssue.PACKAGE_FEATURES)
    features = _metadata_table(features, MetadataIssue.PACKAGE_FEATURES)
    normalized_features = {
        feature: _string_list(values, f"features.{feature}")
        for feature, values in sorted(features.items())
        if isinstance(feature, str)
    }
    return {
        "authors": _string_list(package_table.get("authors"), "package.authors"),
        "badges": badges,
        "categories": _string_list(package_table.get("categories"), "categories"),
        "deps": _dependency_records(manifest),
        "description": package_table.get("description"),
        "documentation": package_table.get("documentation"),
        "features": normalized_features,
        "homepage": package_table.get("homepage"),
        "keywords": _string_list(package_table.get("keywords"), "keywords"),
        "license": package_table.get("license"),
        "license_file": package_table.get("license-file"),
        "links": package_table.get("links"),
        "name": name,
        "readme": packaged.readme,
        "readme_file": packaged.readme_file,
        "repository": package_table.get("repository"),
        "rust_version": package_table.get("rust-version"),
        "vers": version,
    }
