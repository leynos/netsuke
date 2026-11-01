"""Specify candidate validation and Cargo registry metadata derivation."""

import dataclasses
import hashlib
import json
import typing as typ

import pytest
from crates_io_candidate_cli import EXIT_OK, EXIT_REJECTED, main
from crates_io_candidate_support import (
    BIN_NAME,
    COMMIT,
    PACKAGE_NAME,
    TAG,
    VERSION,
    archive_bytes,
    write_candidate,
)

EXPECTED_README = "# Netsuke\n\nPackaged documentation.\n"

if typ.TYPE_CHECKING:
    import pathlib


@dataclasses.dataclass(frozen=True, slots=True)
class CheckExpectation:
    """Carry workflow identity values passed to the candidate CLI."""

    tag: str = TAG
    commit: str = COMMIT
    package: str = PACKAGE_NAME
    version: str = VERSION


@dataclasses.dataclass(frozen=True, slots=True)
class IdentityMismatch:
    """Describe one identity field and its expected CLI argument."""

    identity_field: str
    identity_value: object
    expectation_field: str
    expectation_value: str
    diagnostic: str


DEFAULT_EXPECTATION = CheckExpectation()


def _check_args(
    record: pathlib.Path,
    directory: pathlib.Path,
    output: pathlib.Path,
    expectation: CheckExpectation = DEFAULT_EXPECTATION,
) -> list[str]:
    """Build the validated-candidate CLI invocation."""
    return [
        "check",
        "--identity",
        str(record),
        "--archive-directory",
        str(directory),
        "--tag",
        expectation.tag,
        "--commit",
        expectation.commit,
        "--package-name",
        expectation.package,
        "--version",
        expectation.version,
        "--output",
        str(output),
    ]


def test_check_records_one_matching_unchanged_archive(tmp_path: pathlib.Path) -> None:
    """Accept the package artifact and retain its exact bytes and digest."""
    record, archive = write_candidate(tmp_path)
    original = archive.read_bytes()
    output = tmp_path / "candidate.json"

    status = main(_check_args(record, tmp_path, output))

    assert status == EXIT_OK, "the matching package artifact must validate"
    candidate = json.loads(output.read_text(encoding="utf-8"))
    assert candidate["archive"] == str(archive), "record the selected archive path"
    assert candidate["crate"] == archive.name, "record its flat basename"
    assert candidate["sha256"] == hashlib.sha256(original).hexdigest(), (
        "record the digest of the retained bytes"
    )
    assert archive.read_bytes() == original, "validation must not rewrite the archive"


@pytest.mark.parametrize(
    "mismatch",
    [
        pytest.param(
            IdentityMismatch("tag", None, "tag", TAG, "a rehearsal identity"),
            id="rehearsal-identity",
        ),
        pytest.param(
            IdentityMismatch(
                "commit",
                "b" * 40,
                "commit",
                COMMIT,
                "candidate identity mismatch",
            ),
            id="commit-mismatch",
        ),
        pytest.param(
            IdentityMismatch(
                "package_name",
                "other-crate",
                "package",
                PACKAGE_NAME,
                "candidate identity mismatch",
            ),
            id="package-mismatch",
        ),
        pytest.param(
            IdentityMismatch(
                "version", "9.9.9", "version", VERSION, "invalid package identity"
            ),
            id="version-mismatch",
        ),
    ],
)
def test_check_rejects_rehearsal_and_identity_mismatches(
    tmp_path: pathlib.Path,
    mismatch: IdentityMismatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Refuse a rehearsal or a candidate that differs from workflow identity."""
    identity = {
        "tag": TAG,
        "commit": COMMIT,
        "is_prerelease": False,
        "package_name": PACKAGE_NAME,
        "version": VERSION,
        "bin_name": BIN_NAME,
    }
    identity[mismatch.identity_field] = mismatch.identity_value
    record, _ = write_candidate(tmp_path, identity=identity)
    output = tmp_path / "candidate.json"
    expectation = dataclasses.replace(
        CheckExpectation(), **{mismatch.expectation_field: mismatch.expectation_value}
    )

    status = main(_check_args(record, tmp_path, output, expectation))

    assert status == EXIT_REJECTED, "reject rehearsal and mismatched identities"
    diagnostic = capsys.readouterr().err
    assert mismatch.diagnostic in diagnostic, "explain why the identity was rejected"
    assert not output.exists(), "a refused candidate must produce no validated record"


@pytest.mark.parametrize(
    "crate_name",
    ["../netsuke-build-1.2.3.crate", "nested/crate.crate", "bad\\name.crate"],
)
def test_check_rejects_non_flat_archive_basename(
    tmp_path: pathlib.Path, crate_name: str
) -> None:
    """Validate the recorded basename before joining it to the artifact path."""
    record, _ = write_candidate(tmp_path, crate_name=crate_name)
    output = tmp_path / "candidate.json"

    assert main(_check_args(record, tmp_path, output)) == EXIT_REJECTED, (
        "reject path-shaped archive names"
    )
    assert not output.exists(), "do not emit a candidate for a nested archive name"


def test_check_refuses_multiple_crate_archives(tmp_path: pathlib.Path) -> None:
    """Reject artifact ambiguity instead of selecting an unrelated archive."""
    record, _ = write_candidate(tmp_path)
    _ = (tmp_path / "unrelated-0.0.1.crate").write_bytes(b"other")
    output = tmp_path / "candidate.json"

    assert main(_check_args(record, tmp_path, output)) == EXIT_REJECTED, (
        "reject a directory with more than one crate archive"
    )


def test_check_refuses_digest_and_packaged_manifest_mismatches(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Require the candidate digest and packaged Cargo identity to agree."""
    record, _ = write_candidate(
        tmp_path, crate_bytes=archive_bytes(manifest_version="9.9.9")
    )
    output = tmp_path / "candidate.json"

    assert main(_check_args(record, tmp_path, output)) == EXIT_REJECTED, (
        "reject an archive whose packaged version disagrees with the candidate"
    )
    assert "packaged manifest identity" in capsys.readouterr().err, (
        "reach packaged-manifest validation with a matching candidate digest"
    )

    archive = tmp_path / f"{PACKAGE_NAME}-{VERSION}.crate"
    body = archive_bytes()
    _ = archive.write_bytes(body)
    payload = json.loads(record.read_text(encoding="utf-8"))
    payload["sha256"] = "0" * 64
    _ = record.write_text(json.dumps(payload), encoding="utf-8")

    assert main(_check_args(record, tmp_path, output)) == EXIT_REJECTED, (
        "reject a candidate digest that does not match the archive"
    )


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
