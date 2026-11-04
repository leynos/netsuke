"""Specify Cargo registry metadata derived from a packaged crate archive."""

import json
import typing as typ

import pytest
from crates_io_candidate_cli import EXIT_OK, main
from crates_io_candidate_support import (
    PACKAGE_NAME,
    VERSION,
    archive_bytes,
    manifest_text,
    write_candidate,
)
from crates_io_upload_metadata import MetadataError, MetadataIssue, package_metadata

EXPECTED_README = "# Netsuke\n\nPackaged documentation.\n"

if typ.TYPE_CHECKING:
    import pathlib


def test_metadata_normalizes_manifest_and_all_dependency_kinds(
    tmp_path: pathlib.Path,
) -> None:
    """Derive Cargo's publish record in memory without changing archive bytes."""
    _, archive = write_candidate(tmp_path)
    original = archive.read_bytes()

    status = main([
        "metadata",
        "--archive",
        str(archive),
        "--output",
        str(tmp_path / "metadata.json"),
    ])

    assert status == EXIT_OK, "metadata must be derived from a valid crate archive"
    metadata = json.loads((tmp_path / "metadata.json").read_text(encoding="utf-8"))
    assert metadata["name"] == PACKAGE_NAME, "read the package name from the archive"
    assert metadata["vers"] == VERSION, "read the package version from the archive"
    assert metadata["readme"] == EXPECTED_README, "include packaged README contents"
    assert metadata["readme_file"] == "README.md", "retain the packaged README path"
    dependencies = metadata["deps"]
    serde = next(
        dependency
        for dependency in dependencies
        if dependency["name"] == "serde"
        and dependency["target"] is None
        and dependency["explicit_name_in_toml"] is None
    )
    renamed_serde = next(
        dependency
        for dependency in dependencies
        if dependency["explicit_name_in_toml"] == "renamed-serde"
    )
    assert serde["default_features"] is False, "preserve disabled default features"
    assert serde["features"] == ["derive"], "preserve requested dependency features"
    assert serde["optional"] is False, "mark required dependencies as non-optional"
    assert renamed_serde["optional"] is True, "preserve optional renamed dependencies"
    rstest_dependency = next(
        dependency for dependency in dependencies if dependency["name"] == "rstest"
    )
    assert rstest_dependency["version_req"] == "^0.26", (
        "preserve development dependency requirements"
    )
    cc_dependency = next(
        dependency for dependency in dependencies if dependency["name"] == "cc"
    )
    assert cc_dependency["kind"] == "build", (
        "mark build dependencies with the correct kind"
    )
    assert next(
        dependency for dependency in dependencies if dependency["name"] == "windows-sys"
    )["features"] == ["Win32_Foundation"], (
        "preserve target-specific dependency features"
    )
    assert metadata["links"] == "netsuke_native", "preserve the native links field"
    assert archive.read_bytes() == original, (
        "metadata derivation must retain exact bytes"
    )


def test_metadata_rejects_missing_archive(tmp_path: pathlib.Path) -> None:
    """Classify an absent archive as an archive-read failure."""
    with pytest.raises(MetadataError) as error:
        package_metadata(tmp_path / "missing.crate")

    assert error.value.issue is MetadataIssue.ARCHIVE_READ, (
        "classify unreadable candidate archives"
    )


def test_metadata_rejects_invalid_archive(tmp_path: pathlib.Path) -> None:
    """Classify malformed tar bytes as an archive-read failure."""
    archive = tmp_path / "invalid.crate"
    _ = archive.write_bytes(b"not a gzip tar archive")

    with pytest.raises(MetadataError) as error:
        package_metadata(archive)

    assert error.value.issue is MetadataIssue.ARCHIVE_READ, (
        "classify archives that cannot be opened"
    )


@pytest.mark.parametrize(
    ("manifests", "expected_issue"),
    [
        pytest.param((), MetadataIssue.MANIFEST_COUNT, id="missing-manifest"),
        pytest.param(
            (manifest_text(), manifest_text()),
            MetadataIssue.MANIFEST_COUNT,
            id="duplicate-manifest",
        ),
        pytest.param(("[package\n",), MetadataIssue.ARCHIVE_READ, id="malformed-toml"),
        pytest.param(
            ("[workspace]\nmembers = []\n",),
            MetadataIssue.PACKAGE_TABLE,
            id="missing-package-table",
        ),
        pytest.param(
            ("package = 1\n",),
            MetadataIssue.PACKAGE_TABLE,
            id="invalid-package-table",
        ),
        pytest.param(
            ("[package]\nname = 1\nversion = '1.0.0'\n",),
            MetadataIssue.PACKAGE_IDENTITY,
            id="invalid-package-identity",
        ),
    ],
)
def test_metadata_rejects_malformed_packaged_manifests(
    tmp_path: pathlib.Path,
    manifests: tuple[str, ...],
    expected_issue: MetadataIssue,
) -> None:
    """Report the precise issue for invalid packaged Cargo manifests."""
    archive = tmp_path / "malformed.crate"
    _ = archive.write_bytes(archive_bytes(manifests=manifests))

    with pytest.raises(MetadataError) as error:
        package_metadata(archive)

    assert error.value.issue is expected_issue, "retain the manifest issue category"
