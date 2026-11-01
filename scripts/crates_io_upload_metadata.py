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


def _string_list(value: object, field: str) -> list[str]:
    """Return a string array from a manifest field, or an empty array."""
    if value is None:
        return []
    if not isinstance(value, list) or any(not isinstance(item, str) for item in value):
        raise MetadataError(MetadataIssue.STRING_ARRAY, (field,))
    return value


def _readme_file(
    package: tarfile.TarFile, root: str, package_table: dict[str, object]
) -> tuple[str | None, str | None]:
    """Read packaged README contents and return the relative path."""
    configured = package_table.get("readme")
    if configured is False:
        return None, None
    candidates = [configured] if isinstance(configured, str) else list(_AUTO_READMES)
    selected: str | None = None
    for candidate in candidates:
        relative = pathlib.PurePosixPath(candidate)
        if not _is_safe_readme_path(candidate, relative):
            raise MetadataError(MetadataIssue.README_PATH, (candidate,))
        member_name = f"{root}/{relative.as_posix()}"
        member = next(
            (item for item in package.getmembers() if item.name == member_name), None
        )
        if member is not None and member.isfile():
            selected = member_name
            break
    if selected is None:
        if isinstance(configured, str):
            raise MetadataError(MetadataIssue.README_ABSENT, (configured,))
        return None, None
    member_file = package.extractfile(selected)
    if member_file is None:
        raise MetadataError(MetadataIssue.README_READ, (selected,))
    return member_file.read().decode("utf-8"), selected.removeprefix(f"{root}/")


def _is_safe_readme_path(candidate: str, path: pathlib.PurePosixPath) -> bool:
    """Reject absolute, parent-relative or platform-ambiguous README paths."""
    return not path.is_absolute() and ".." not in path.parts and "\\" not in candidate


def _dependency(
    name: str, specification: object, kind: str, target: str | None
) -> dict[str, object]:
    """Normalize one Cargo dependency into registry upload fields."""
    match specification:
        case str() as version:
            fields: dict[str, object] = {"version": version}
        case dict() as fields:
            pass
        case _:
            raise MetadataError(MetadataIssue.DEPENDENCY_SPEC, (name,))
    version_req = fields.get("version")
    if not isinstance(version_req, str):
        raise MetadataError(MetadataIssue.DEPENDENCY_VERSION, (name,))
    features = _string_list(fields.get("features"), f"dependency {name}.features")
    optional = fields.get("optional", False)
    default_features = fields.get("default-features", True)
    if not isinstance(optional, bool) or not isinstance(default_features, bool):
        raise MetadataError(MetadataIssue.DEPENDENCY_FLAGS, (name,))
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


def _dependency_records(manifest: dict[str, object]) -> list[dict[str, object]]:
    """Flatten normal, development, build and target-specific dependencies."""
    sources: list[tuple[str, dict[str, object], str | None]] = []
    for table_name, kind in _DEPENDENCY_KINDS:
        table = manifest.get(table_name, {})
        if not isinstance(table, dict):
            raise MetadataError(MetadataIssue.MANIFEST_TABLE, (table_name,))
        sources.append((kind, table, None))
    targets = manifest.get("target", {})
    if not isinstance(targets, dict):
        raise MetadataError(MetadataIssue.TARGET_TABLE)
    for target, raw_tables in targets.items():
        if not isinstance(target, str) or not isinstance(raw_tables, dict):
            raise MetadataError(MetadataIssue.TARGET_SHAPE)
        for table_name, kind in _DEPENDENCY_KINDS:
            table = raw_tables.get(table_name, {})
            if not isinstance(table, dict):
                raise MetadataError(MetadataIssue.TARGET_DEPENDENCIES, (table_name,))
            sources.append((kind, table, target))
    return [
        _dependency(name, specification, kind, target)
        for kind, table, target in sources
        for name, specification in sorted(table.items())
    ]


def _read_packaged_manifest(archive_path: pathlib.Path) -> PackagedManifest:
    """Read the sole packaged manifest and README without extracting files."""
    try:
        archive_bytes = archive_path.read_bytes()
        with tarfile.open(fileobj=io.BytesIO(archive_bytes), mode="r:gz") as archive:
            manifests = [
                member
                for member in archive.getmembers()
                if member.name.endswith("/Cargo.toml")
            ]
            if len(manifests) != 1 or not manifests[0].isfile():
                raise MetadataError(MetadataIssue.MANIFEST_COUNT)
            manifest_file = archive.extractfile(manifests[0])
            if manifest_file is None:
                raise MetadataError(MetadataIssue.MANIFEST_READ)
            manifest = tomllib.loads(manifest_file.read().decode("utf-8"))
            package = manifest.get("package")
            if not isinstance(package, dict):
                raise MetadataError(MetadataIssue.PACKAGE_TABLE)
            root = manifests[0].name.removesuffix("/Cargo.toml")
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
    if not isinstance(badges, dict) or not isinstance(features, dict):
        raise MetadataError(MetadataIssue.PACKAGE_FEATURES)
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
