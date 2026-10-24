"""Exercise the inventory-verification CLI the verifier job invokes.

The three agreements it checks are proved in their own modules. These tests pin
the composition: the declared set and the trusted record are read from the
files their stages wrote, a missing archive is reported as a missing archive
rather than as a malformed sidecar, and an asset whose bytes nobody vouched
for stops the run. No test here opens a socket.

The two substitution cases are the reason the trusted record exists: each
replaces bytes *and* leaves every sidecar in agreement, the shape the older
two-agreement check could not see.
"""

import json
import typing as typ

import pytest
import release_digest
import release_digest_record
import release_inventory
from release_staging_support import (
    ARCHIVE,
    PACKAGE,
    SIDECAR,
)
from release_staging_support import (
    stage_archive as _stage,
)
from release_staging_support import stage_package as _stage_package
from release_staging_support import trusted_digests as _trusted
from release_staging_support import write_declaration as _declaration
from release_staging_support import write_trusted_record as _write_record
from verify_release_inventory_cli import (
    EXIT_INPUT_UNREADABLE,
    EXIT_INVENTORY_REJECTED,
    EXIT_OK,
    build_parser,
    main,
    verify,
)

if typ.TYPE_CHECKING:
    import pathlib


def test_verify_accepts_the_declared_assets(tmp_path: pathlib.Path) -> None:
    """Accept an archive with a matching sidecar and report each asset's own digest.

    The sidecar entry is asserted separately and deliberately. Returning the
    archive's digest under the sidecar's name type-checks and looks right in a
    two-key mapping, but GitHub reports a digest for the sidecar file itself,
    so the finalizer would compare the wrong value. Computing the two digests
    from the staged files rather than from the CLI's own output keeps the
    assertion independent of the code under test.
    """
    staging = tmp_path / "dist"
    _stage(staging)

    verified = verify(staging, expected=[ARCHIVE, SIDECAR], trusted=_trusted(staging))

    assert sorted(verified) == [ARCHIVE, SIDECAR], (
        f"both assets must be keyed by name, got {sorted(verified)}"
    )
    assert verified[ARCHIVE] == release_digest.digest_of_file(staging / ARCHIVE), (
        "the archive's digest must be the digest of its bytes"
    )
    assert verified[SIDECAR] == release_digest.digest_of_file(staging / SIDECAR), (
        "the sidecar's digest must be the digest of the sidecar file, not of "
        "the archive it describes"
    )
    assert verified[SIDECAR] != verified[ARCHIVE], (
        "the two entries must differ, or this assertion cannot tell the bug "
        "from the fix"
    )


def test_verify_reports_a_missing_archive(tmp_path: pathlib.Path) -> None:
    """Reject a release carrying a sidecar whose archive never arrived.

    This is the 2026-10-12 failure: every staged file passed inspection, and
    the release was unusable because the archives were the files that were
    missing. The declaration is what makes the absence visible.
    """
    staging = tmp_path / "dist"
    staging.mkdir()
    _ = (staging / SIDECAR).write_text("ab  dist/x\n", encoding="utf-8")

    with pytest.raises(release_inventory.MissingAssetError):
        _ = verify(
            staging,
            expected=[ARCHIVE, SIDECAR],
            trusted={ARCHIVE: "ab" * 32, SIDECAR: "ab" * 32},
        )


def test_verify_reports_an_unexpected_archive(tmp_path: pathlib.Path) -> None:
    """Reject an asset the release never declared."""
    staging = tmp_path / "dist"
    _stage(staging)
    _ = (staging / "extra.tar.gz").write_bytes(b"extra")

    with pytest.raises(release_inventory.UnexpectedAssetError):
        _ = verify(staging, expected=[ARCHIVE, SIDECAR], trusted=_trusted(staging))


def test_verify_rejects_a_sidecar_describing_different_bytes(
    tmp_path: pathlib.Path,
) -> None:
    """Stop when the sidecar's digest is not the archive's, not merely well formed."""
    staging = tmp_path / "dist"
    _stage(staging, sidecar_body=b"cd" * 32)

    with pytest.raises(release_digest.DigestMismatchError):
        _ = verify(staging, expected=[ARCHIVE, SIDECAR], trusted=_trusted(staging))


def test_verify_rejects_a_replaced_operating_system_package(
    tmp_path: pathlib.Path,
) -> None:
    """Stop when a downloaded package differs from the bytes the build staged.

    The package is replaced outright. Nothing inside the download disagrees —
    a package has no sidecar — so this is the case only the trusted record can
    see, and it is the reason that record exists.
    """
    staged = tmp_path / "staged"
    _stage_package(staged, b"the package the build produced")
    trusted = _trusted(staged)

    downloaded = tmp_path / "downloaded"
    _stage_package(downloaded, b"a package somebody else produced")

    with pytest.raises(release_digest_record.RecordedDigestMismatchError):
        _ = verify(downloaded, expected=[PACKAGE], trusted=trusted)


def test_verify_rejects_a_replaced_archive_and_sidecar_pair(
    tmp_path: pathlib.Path,
) -> None:
    """Stop when an archive and its sidecar are replaced together.

    Both files are rewritten so the sidecar still describes the archive beside
    it. The two older agreements — inventory exactness and sidecar agreement —
    both hold over this download, which is what makes the pair-swap the case
    those agreements cannot detect and the trusted record must.
    """
    staged = tmp_path / "staged"
    _stage(staged)
    trusted = _trusted(staged)

    downloaded = tmp_path / "downloaded"
    _stage(downloaded)
    replaced = b"a different archive entirely"
    _ = (downloaded / ARCHIVE).write_bytes(replaced)
    digest = release_digest.digest_of_file(downloaded / ARCHIVE)
    _ = (downloaded / SIDECAR).write_text(
        f"{digest}  dist/{ARCHIVE}\n", encoding="utf-8"
    )

    with pytest.raises(release_digest_record.RecordedDigestMismatchError):
        _ = verify(downloaded, expected=[ARCHIVE, SIDECAR], trusted=trusted)


def test_verify_rejects_a_record_that_omits_a_declared_asset(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse a trusted record that leaves an asset uncompared.

    A record covering only part of the declaration would let the omitted asset
    through whatever its bytes are, so the omission is the failure.
    """
    staging = tmp_path / "dist"
    _stage(staging)
    partial = {ARCHIVE: _trusted(staging)[ARCHIVE]}

    with pytest.raises(release_digest_record.DigestRecordError):
        _ = verify(staging, expected=[ARCHIVE, SIDECAR], trusted=partial)


def test_the_cli_records_the_verified_digests(tmp_path: pathlib.Path) -> None:
    """Write the digests the release was verified against."""
    staging = tmp_path / "dist"
    _stage(staging)
    declaration = _declaration(tmp_path / "declared.txt", [ARCHIVE, SIDECAR])
    trusted = _write_record(tmp_path / "trusted.json", _trusted(staging))
    destination = tmp_path / "verified.json"

    code = main([
        "--directory",
        str(staging),
        "--expected",
        str(declaration),
        "--trusted",
        str(trusted),
        "--output",
        str(destination),
    ])

    assert code == EXIT_OK, f"a complete release must verify, got {code}"
    record = json.loads(destination.read_text(encoding="utf-8"))
    assert sorted(record["assets"]) == [ARCHIVE, SIDECAR], (
        f"both assets must be recorded by name, got {sorted(record['assets'])}"
    )


def test_the_cli_rejects_a_replaced_package_and_writes_no_record(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Fail closed on a substituted package, naming it, leaving no record."""
    staged = tmp_path / "staged"
    _stage_package(staged, b"the package the build produced")
    trusted = _write_record(tmp_path / "trusted.json", _trusted(staged))
    declaration = _declaration(tmp_path / "declared.txt", [PACKAGE])

    downloaded = tmp_path / "downloaded"
    _stage_package(downloaded, b"a package somebody else produced")
    destination = tmp_path / "verified.json"

    code = main([
        "--directory",
        str(downloaded),
        "--expected",
        str(declaration),
        "--trusted",
        str(trusted),
        "--output",
        str(destination),
    ])

    assert code == EXIT_INVENTORY_REJECTED, (
        f"a substituted package must be refused, got {code}"
    )
    assert not destination.exists(), "a rejected inventory must leave no record"
    assert PACKAGE in capsys.readouterr().err, (
        "the diagnostic must name the substituted asset"
    )


def test_the_cli_rejects_an_incomplete_release_and_writes_no_record(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Fail closed, naming the disagreement, and leave no record behind."""
    staging = tmp_path / "dist"
    staging.mkdir()
    _ = (staging / SIDECAR).write_text("ab  dist/x\n", encoding="utf-8")
    declaration = _declaration(tmp_path / "declared.txt", [ARCHIVE, SIDECAR])
    trusted = _write_record(
        tmp_path / "trusted.json", {ARCHIVE: "ab" * 32, SIDECAR: "ab" * 32}
    )
    destination = tmp_path / "verified.json"

    code = main([
        "--directory",
        str(staging),
        "--expected",
        str(declaration),
        "--trusted",
        str(trusted),
        "--output",
        str(destination),
    ])

    assert code == EXIT_INVENTORY_REJECTED, (
        f"an incomplete release must be refused, got {code}"
    )
    assert not destination.exists(), "a rejected inventory must leave no record"
    assert ARCHIVE in capsys.readouterr().err, (
        "the diagnostic must name the missing asset"
    )


def test_the_declaration_ignores_blank_lines(tmp_path: pathlib.Path) -> None:
    """Read the declaration leniently, so a trailing newline is not an asset."""
    staging = tmp_path / "dist"
    _stage(staging)
    declaration = tmp_path / "declared.txt"
    _ = declaration.write_text(f"\n{ARCHIVE}\n\n{SIDECAR}\n\n", encoding="utf-8")
    trusted = _write_record(tmp_path / "trusted.json", _trusted(staging))
    destination = tmp_path / "verified.json"

    code = main([
        "--directory",
        str(staging),
        "--expected",
        str(declaration),
        "--trusted",
        str(trusted),
        "--output",
        str(destination),
    ])

    assert code == EXIT_OK, f"blank lines must not be read as asset names, got {code}"


def test_the_cli_reports_an_unreadable_declaration_separately(
    tmp_path: pathlib.Path,
) -> None:
    """Distinguish a bad path from a bad release, which sends an operator elsewhere."""
    destination = tmp_path / "verified.json"

    code = main([
        "--directory",
        str(tmp_path / "dist"),
        "--expected",
        str(tmp_path / "absent.txt"),
        "--trusted",
        str(tmp_path / "absent.json"),
        "--output",
        str(destination),
    ])

    assert code == EXIT_INPUT_UNREADABLE, (
        f"an unreadable declaration must not be a rejection, got {code}"
    )
    assert not destination.exists(), "nothing was verified, so nothing is recorded"


def test_the_cli_reports_an_unreadable_trusted_record_as_unreadable(
    tmp_path: pathlib.Path,
) -> None:
    """Distinguish an absent trusted record from a malformed one.

    An absent path is a wiring fault while a malformed record is something the
    build wrote. The declaration is readable on purpose so this test reaches
    the trusted record; the other unreadable-input test makes both absent and
    returns at the declaration.
    """
    staging = tmp_path / "dist"
    _stage(staging)
    declaration = _declaration(tmp_path / "declared.txt", [ARCHIVE, SIDECAR])
    destination = tmp_path / "verified.json"

    code = main([
        "--directory",
        str(staging),
        "--expected",
        str(declaration),
        "--trusted",
        str(tmp_path / "absent.json"),
        "--output",
        str(destination),
    ])

    assert code == EXIT_INPUT_UNREADABLE, (
        f"an absent trusted record is an input fault, not a rejection, got {code}"
    )
    assert not destination.exists(), "nothing verified, so nothing is recorded"


def test_the_cli_reports_a_malformed_trusted_record_as_a_rejection(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Reject a record that parses but cannot support a decision."""
    staging = tmp_path / "dist"
    _stage(staging)
    declaration = _declaration(tmp_path / "declared.txt", [ARCHIVE, SIDECAR])
    trusted = tmp_path / "trusted.json"
    _ = trusted.write_text('{"assets": {}}\n', encoding="utf-8")
    destination = tmp_path / "verified.json"

    code = main([
        "--directory",
        str(staging),
        "--expected",
        str(declaration),
        "--trusted",
        str(trusted),
        "--output",
        str(destination),
    ])

    assert code == EXIT_INVENTORY_REJECTED, (
        f"an empty trusted record must be refused, not accepted, got {code}"
    )
    assert not destination.exists(), "a rejected record must leave no record"
    assert "trusted digest record" in capsys.readouterr().err, (
        "the diagnostic must name the trusted record as the fault"
    )


@pytest.mark.parametrize(
    "missing", ["--directory", "--expected", "--trusted", "--output"]
)
def test_the_cli_requires_its_four_inputs(missing: str) -> None:
    """Refuse to guess any of the four, since each names a different file."""
    complete = {
        "--directory": "dist",
        "--expected": "declared.txt",
        "--trusted": "trusted.json",
        "--output": "verified.json",
    }
    argv = [
        part
        for flag, value in complete.items()
        if flag != missing
        for part in (flag, value)
    ]

    with pytest.raises(SystemExit):
        _ = build_parser().parse_args(argv)
