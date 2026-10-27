"""Exercise the release-inventory verification command's file contract.

The command reads the declaration and trusted digest record, verifies each
asset, and writes a result only after the whole release passes.
"""

import json
import typing as typ
from unittest import mock

import pytest
import verify_release_inventory_cli as verifier
from release_staging_support import ARCHIVE, PACKAGE, SIDECAR
from release_staging_support import stage_archive as _stage
from release_staging_support import stage_package as _stage_package
from release_staging_support import trusted_digests as _trusted
from release_staging_support import write_declaration as _declaration
from release_staging_support import write_trusted_record as _write_record
from verify_release_inventory_cli import (
    EXIT_INPUT_UNREADABLE,
    EXIT_INVENTORY_REJECTED,
    EXIT_OK,
    EXIT_OUTPUT_UNWRITABLE,
    build_parser,
    main,
)

if typ.TYPE_CHECKING:
    import pathlib


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


@pytest.mark.parametrize(
    "failure", ["directory", "metadata", "archive", "sidecar", "output"]
)
def test_filesystem_failures_are_diagnosed_and_preserve_existing_output(
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    failure: str,
) -> None:
    """Diagnose filesystem failures without replacing an earlier record."""
    staging = tmp_path / "dist"
    _stage(staging)
    declaration = _declaration(tmp_path / "declared.txt", [ARCHIVE, SIDECAR])
    trusted = _write_record(tmp_path / "trusted.json", _trusted(staging))
    destination = tmp_path / "verified.json"
    _ = destination.write_text("previous record\n", encoding="utf-8")
    fault = mock.Mock(side_effect=PermissionError("simulated access failure"))

    match failure:
        case "directory":
            staging = tmp_path / "absent"
        case "metadata":
            entry = mock.MagicMock()
            entry.is_file.return_value = True
            entry.stat.side_effect = PermissionError("simulated access failure")
            scan = mock.MagicMock()
            scan.__enter__.return_value = [entry]
            monkeypatch.setattr(verifier.os, "scandir", mock.Mock(return_value=scan))
        case "archive":
            monkeypatch.setattr(verifier.release_digest_files, "digest_of_file", fault)
        case "sidecar":
            monkeypatch.setattr(
                verifier.release_digest_files, "verify_file_against_sidecar", fault
            )
        case _:
            monkeypatch.setattr(verifier.pathlib.Path, "replace", fault)

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

    assert code == (
        EXIT_OUTPUT_UNWRITABLE if failure == "output" else EXIT_INPUT_UNREADABLE
    ), f"{failure} failure returned unexpected status {code}"
    assert destination.read_text(encoding="utf-8") == "previous record\n", (
        f"{failure} failure changed the previous record"
    )
    diagnostic = capsys.readouterr().err
    assert ("unwritable" if failure == "output" else "unreadable") in diagnostic, (
        f"{failure} failure lacked a diagnostic: {diagnostic}"
    )
