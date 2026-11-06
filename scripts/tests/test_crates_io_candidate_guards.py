"""Guard the candidate record and packaged README boundaries.

``test_crates_io_candidate_cli.py`` covers the accepted candidate and identity
mismatches; ``test_crates_io_candidate_metadata.py`` covers the registry
metadata derived from its archive. This module covers the rejection
paths that sit beside them: archive file-kind guards, damaged identity records,
malformed recorded digests, and the three packaged-README selection outcomes.
It lives apart so each module stays within the repository's file-length cap.
"""

import dataclasses
import json
import typing as typ

import pytest
from crates_io_candidate_cli import EXIT_REJECTED, main
from crates_io_candidate_support import (
    COMMIT,
    PACKAGE_NAME,
    TAG,
    VERSION,
    CandidateOverrides,
    archive_bytes,
    manifest_text,
    tar_bytes,
    write_candidate,
)
from crates_io_upload_metadata import MetadataError, MetadataIssue, package_metadata

if typ.TYPE_CHECKING:
    import pathlib

EXPECTED_README = "# Netsuke\n\nPackaged documentation.\n"


@dataclasses.dataclass(frozen=True, slots=True)
class CandidatePaths:
    """Carry the three filesystem roles the candidate CLI is given.

    The record and the archive directory are always the fixture's own identity
    record and its parent directory, while the output path names a file the run
    must not create. Keeping them together means the rejection helper takes one
    argument per concern instead of one per path.
    """

    record: pathlib.Path
    directory: pathlib.Path
    output: pathlib.Path


def _fixture_paths(record: pathlib.Path, directory: pathlib.Path) -> CandidatePaths:
    """Pair a fixture record with the conventional candidate output path."""
    return CandidatePaths(record, directory, directory / "candidate.json")


def _check_args(paths: CandidatePaths) -> list[str]:
    """Build the validated-candidate CLI invocation for the fixture identity."""
    return [
        "check",
        "--identity",
        str(paths.record),
        "--archive-directory",
        str(paths.directory),
        "--tag",
        TAG,
        "--commit",
        COMMIT,
        "--package-name",
        PACKAGE_NAME,
        "--version",
        VERSION,
        "--output",
        str(paths.output),
    ]


def _assert_rejected_without_output(
    capsys: pytest.CaptureFixture[str], paths: CandidatePaths, expected: str
) -> None:
    """Require a classified rejection that leaves no candidate record."""
    assert main(_check_args(paths)) == EXIT_REJECTED, (
        f"reject the candidate for {expected!r}"
    )
    assert expected in capsys.readouterr().err, f"report {expected!r} as the reason"
    assert not paths.output.exists(), (
        "a rejected candidate must not write an output record"
    )


def test_check_rejects_symlinked_archive(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Refuse an archive that is a symlink rather than the retained file."""
    record, archive = write_candidate(tmp_path)
    target = tmp_path / "retained" / archive.name
    target.parent.mkdir()
    _ = target.write_bytes(archive.read_bytes())
    archive.unlink()
    archive.symlink_to(target)

    _assert_rejected_without_output(
        capsys, _fixture_paths(record, tmp_path), "not a regular file"
    )


def test_check_rejects_directory_named_like_the_archive(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Refuse a directory entry that matches the recorded archive name."""
    record, archive = write_candidate(tmp_path)
    archive.unlink()
    archive.mkdir()

    _assert_rejected_without_output(
        capsys, _fixture_paths(record, tmp_path), "not a regular file"
    )


def test_check_rejects_a_manifest_outside_the_package_root(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Bind the packaged manifest to the release's own package-version root."""
    record, _ = write_candidate(
        tmp_path,
        overrides=CandidateOverrides(crate_bytes=archive_bytes(root="other")),
    )

    _assert_rejected_without_output(
        capsys, _fixture_paths(record, tmp_path), "packaged manifest is rooted at"
    )


def test_check_rejects_multiple_packaged_manifests(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Refuse an archive holding a second manifest outside the release root."""
    manifest = manifest_text().encode()
    body = tar_bytes((
        (f"{PACKAGE_NAME}-{VERSION}/Cargo.toml", manifest),
        ("other/Cargo.toml", manifest),
    ))
    record, _ = write_candidate(
        tmp_path, overrides=CandidateOverrides(crate_bytes=body)
    )

    _assert_rejected_without_output(
        capsys, _fixture_paths(record, tmp_path), "exactly one packaged Cargo.toml"
    )


def test_check_rejects_an_archive_holding_no_manifest(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Refuse an archive that packages no manifest at the expected depth."""
    body = tar_bytes(((f"{PACKAGE_NAME}-{VERSION}/README.md", b"# Netsuke\n"),))
    record, _ = write_candidate(
        tmp_path, overrides=CandidateOverrides(crate_bytes=body)
    )

    _assert_rejected_without_output(
        capsys, _fixture_paths(record, tmp_path), "exactly one packaged Cargo.toml"
    )


def test_check_rejects_a_non_regular_manifest_member(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Refuse a manifest member that is a directory rather than a file."""
    body = tar_bytes(((f"{PACKAGE_NAME}-{VERSION}/Cargo.toml", None),))
    record, _ = write_candidate(
        tmp_path, overrides=CandidateOverrides(crate_bytes=body)
    )

    # The member is present and unique, so the count diagnostic would describe
    # a valid selection. The rejection must name the file kind instead.
    _assert_rejected_without_output(
        capsys, _fixture_paths(record, tmp_path), "is not a regular file"
    )


def test_check_rejects_missing_identity_object(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Refuse a candidate record that carries no identity object."""
    record, _ = write_candidate(tmp_path)
    payload = json.loads(record.read_text(encoding="utf-8"))
    del payload["identity"]
    _ = record.write_text(json.dumps(payload), encoding="utf-8")

    _assert_rejected_without_output(
        capsys, _fixture_paths(record, tmp_path), "has no identity object"
    )


@pytest.mark.parametrize(
    ("recorded_digest", "expected"),
    [
        pytest.param("uppercase", "lowercase hexadecimal", id="uppercase-digest"),
        pytest.param("non-hex", "lowercase hexadecimal", id="non-hex-digest"),
        pytest.param("missing", "has no SHA-256 digest", id="missing-digest"),
    ],
)
def test_check_rejects_malformed_recorded_digests(
    tmp_path: pathlib.Path,
    capsys: pytest.CaptureFixture[str],
    recorded_digest: str,
    expected: str,
) -> None:
    """Refuse recorded digests that are absent or not lowercase SHA-256 hex."""
    match recorded_digest:
        case "uppercase":
            sha256: object = "A" * 64
        case "non-hex":
            sha256 = "g" * 64
        case _:
            sha256 = None
    record, _ = write_candidate(tmp_path, overrides=CandidateOverrides(sha256=sha256))
    if sha256 is None:
        payload = json.loads(record.read_text(encoding="utf-8"))
        del payload["sha256"]
        _ = record.write_text(json.dumps(payload), encoding="utf-8")

    _assert_rejected_without_output(capsys, _fixture_paths(record, tmp_path), expected)


def _metadata_archive(
    directory: pathlib.Path,
    manifest: str,
    *,
    readme: bytes | None = b"# Netsuke\n\nPackaged documentation.\n",
) -> pathlib.Path:
    """Write one archive holding the supplied manifest and optional README."""
    archive = directory / "metadata.crate"
    _ = archive.write_bytes(archive_bytes(manifests=(manifest,), readme=readme))
    return archive


def test_metadata_honours_a_disabled_readme(tmp_path: pathlib.Path) -> None:
    """Derive no README contents when the manifest disables README packaging."""
    manifest = manifest_text().replace('readme = "README.md"', "readme = false")
    metadata = package_metadata(_metadata_archive(tmp_path, manifest))

    assert metadata["readme"] is None, "readme=false must publish no README body"
    assert metadata["readme_file"] is None, "readme=false must publish no README path"


def test_metadata_rejects_an_absent_explicit_readme(tmp_path: pathlib.Path) -> None:
    """Classify a configured README that the archive does not contain."""
    archive = _metadata_archive(tmp_path, manifest_text(), readme=None)

    with pytest.raises(MetadataError) as error:
        package_metadata(archive)

    assert error.value.issue is MetadataIssue.README_ABSENT, (
        "name the absent configured README"
    )


def test_metadata_falls_back_to_a_default_readme(tmp_path: pathlib.Path) -> None:
    """Select an automatic README when the manifest configures none."""
    manifest = manifest_text().replace('readme = "README.md"\n', "")
    metadata = package_metadata(_metadata_archive(tmp_path, manifest))

    assert metadata["readme"] == EXPECTED_README, (
        "automatic selection must find the packaged README"
    )
    assert metadata["readme_file"] == "README.md", (
        "automatic selection must record the discovered path"
    )


def test_metadata_omits_a_readme_when_none_is_packaged(tmp_path: pathlib.Path) -> None:
    """Derive no README fields when automatic selection finds no candidate."""
    manifest = manifest_text().replace('readme = "README.md"\n', "")
    metadata = package_metadata(_metadata_archive(tmp_path, manifest, readme=None))

    assert metadata["readme"] is None, "an absent README publishes no contents"
    assert metadata["readme_file"] is None, "an absent README publishes no path"
