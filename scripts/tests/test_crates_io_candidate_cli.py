"""Specify the offline validation of a retained crate candidate."""

import dataclasses
import hashlib
import json
import typing as typ

import pytest
from crates_io_candidate_cli import EXIT_OK, EXIT_REJECTED, EXIT_USAGE, main
from crates_io_candidate_support import (
    BIN_NAME,
    COMMIT,
    PACKAGE_NAME,
    TAG,
    VERSION,
    CandidateOverrides,
    archive_bytes,
    write_candidate,
)

EXPECTED_README = "# Netsuke\n\nPackaged documentation.\n"

if typ.TYPE_CHECKING:
    import pathlib


@dataclasses.dataclass(frozen=True, slots=True)
class CheckExpectation:
    """Carry workflow identity values passed to the candidate CLI."""

    tag: str | None = TAG
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
REHEARSAL_EXPECTATION = CheckExpectation(tag=None)


def _identity(*, tag: str | None = TAG) -> dict[str, object]:
    """Build one package identity record with the selected release tag."""
    return {
        "tag": tag,
        "commit": COMMIT,
        "is_prerelease": False,
        "package_name": PACKAGE_NAME,
        "version": VERSION,
        "bin_name": BIN_NAME,
    }


def _check_args(
    record: pathlib.Path,
    directory: pathlib.Path,
    output: pathlib.Path,
    expectation: CheckExpectation = DEFAULT_EXPECTATION,
) -> list[str]:
    """Build the validated-candidate CLI invocation for the expectation's mode."""
    # An expectation without a tag rehearses the candidate; one with a tag
    # publishes it. The mode is derived from the same value the check is
    # expected to agree with, not supplied separately.
    release_mode = (
        ["--rehearsal"] if expectation.tag is None else ["--tag", expectation.tag]
    )
    return [
        "check",
        "--identity",
        str(record),
        "--archive-directory",
        str(directory),
        *release_mode,
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


def test_check_accepts_a_tagless_rehearsal_candidate(
    tmp_path: pathlib.Path,
) -> None:
    """Accept a rehearsal candidate that records no release tag."""
    record, archive = write_candidate(
        tmp_path, overrides=CandidateOverrides(identity=_identity(tag=None))
    )
    output = tmp_path / "candidate.json"

    status = main(_check_args(record, tmp_path, output, REHEARSAL_EXPECTATION))

    assert status == EXIT_OK, "a tagless rehearsal candidate must validate"
    candidate = json.loads(output.read_text(encoding="utf-8"))
    assert candidate["identity"]["tag"] is None, "retain the absent release tag"
    assert candidate["crate"] == archive.name, "record the selected archive"


def test_check_rejects_a_tagged_candidate_during_rehearsal(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Refuse a rehearsal candidate that claims a release tag."""
    record, _ = write_candidate(tmp_path)
    output = tmp_path / "candidate.json"

    status = main(_check_args(record, tmp_path, output, REHEARSAL_EXPECTATION))

    assert status == EXIT_REJECTED, "a rehearsal must not accept a tagged candidate"
    assert "must not record a release tag" in capsys.readouterr().err, (
        "explain that the rehearsal candidate carries a tag"
    )
    assert not output.exists(), "a refused candidate must produce no record"


@pytest.mark.parametrize(
    "release_mode",
    [
        pytest.param([], id="neither-mode"),
        pytest.param(["--tag", TAG, "--rehearsal"], id="both-modes"),
    ],
)
def test_check_requires_exactly_one_release_mode(
    tmp_path: pathlib.Path, release_mode: list[str]
) -> None:
    """Refuse an invocation that does not select exactly one release mode."""
    record, _ = write_candidate(tmp_path)
    output = tmp_path / "candidate.json"
    arguments = [
        "check",
        "--identity",
        str(record),
        "--archive-directory",
        str(tmp_path),
        *release_mode,
        "--commit",
        COMMIT,
        "--package-name",
        PACKAGE_NAME,
        "--version",
        VERSION,
        "--output",
        str(output),
    ]

    assert main(arguments) == EXIT_USAGE, "the release mode must be unambiguous"
    assert not output.exists(), "a usage error must produce no candidate record"


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
    record, _ = write_candidate(
        tmp_path, overrides=CandidateOverrides(identity=identity)
    )
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
    record, _ = write_candidate(
        tmp_path, overrides=CandidateOverrides(crate_name=crate_name)
    )
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
        tmp_path,
        overrides=CandidateOverrides(
            crate_bytes=archive_bytes(manifest_version="9.9.9")
        ),
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
