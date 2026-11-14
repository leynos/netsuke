"""Exercise the verifier's output and non-regular-entry contracts.

The command's decision rules are specified in
``test_verify_release_inventory_command.py``. These cases are about what the
command leaves on disk and about which directory entries it observes at all: a
link or directory must not stand in for an asset, a failed write of any kind
must preserve an earlier record, and a replacement that completes before a
cleanup fault must still be reported as a failure. That last case is why
consumers check the exit status rather than the output's existence.
"""

import json
import tempfile
import typing as typ
from unittest import mock

import pytest
import release_digest_files
import release_digest_record
import verify_release_inventory_cli as verifier
from release_staging_support import ARCHIVE, PACKAGE, SIDECAR
from release_staging_support import stage_archive as _stage
from release_staging_support import trusted_digests as _trusted
from release_staging_support import write_declaration as _declaration
from release_staging_support import write_trusted_record as _write_record
from verify_release_inventory_cli import (
    EXIT_INVENTORY_REJECTED,
    EXIT_OK,
    EXIT_OUTPUT_UNWRITABLE,
    main,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import pathlib
    import types


class _CleanupFaultError(OSError):
    """Report the simulated staging-directory cleanup failure."""


def test_the_cli_rejects_a_replaced_archive_with_a_recomputed_sidecar(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Refuse a download that agrees with itself but not with the build.

    The archive and its sidecar are replaced together, so inventory exactness
    and sidecar agreement both hold. Only the trusted record, produced from the
    build's own tree before upload, can see the substitution.
    """
    staged = tmp_path / "staged"
    _stage(staged)
    trusted = _write_record(tmp_path / "trusted.json", _trusted(staged))
    declaration = _declaration(tmp_path / "declared.txt", [ARCHIVE, SIDECAR])

    downloaded = tmp_path / "downloaded"
    _stage(downloaded)
    _ = (downloaded / ARCHIVE).write_bytes(b"a different archive entirely")
    digest = release_digest_files.digest_of_file(downloaded / ARCHIVE)
    _ = (downloaded / SIDECAR).write_text(
        f"{digest}  dist/{ARCHIVE}\n", encoding="utf-8"
    )
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
        f"a substituted archive and sidecar must be refused, got {code}"
    )
    assert not destination.exists(), "a rejected inventory must leave no record"
    assert ARCHIVE in capsys.readouterr().err, (
        "the diagnostic must name the substituted asset"
    )


@pytest.mark.parametrize("shape", ["symlink", "directory"])
def test_a_declared_name_present_only_nonregularly_is_missing(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str], shape: str
) -> None:
    """Count only regular files, so a link or directory cannot stand in.

    A staged link would otherwise let the verifier hash whatever it points at,
    and a directory would fail later with an opaque read error instead of the
    missing-asset diagnosis an operator needs.
    """
    staging = tmp_path / "dist"
    _stage(staging)
    entry = staging / PACKAGE
    if shape == "symlink":
        entry.symlink_to(staging / ARCHIVE)
    else:
        entry.mkdir()
    declaration = _declaration(tmp_path / "declared.txt", [ARCHIVE, SIDECAR, PACKAGE])
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

    assert code == EXIT_INVENTORY_REJECTED, (
        f"a {shape} standing in for an asset must be refused, got {code}"
    )
    assert not destination.exists(), "nothing was verified, so nothing is recorded"
    assert PACKAGE in capsys.readouterr().err, (
        f"the diagnostic must name the {shape} standing in for the asset"
    )


def test_an_undeclared_directory_or_symlink_is_not_observed(
    tmp_path: pathlib.Path,
) -> None:
    """Ignore entries that are not regular files, declaring neither.

    The workflow downloads into a fresh, empty directory, so an undeclared
    non-regular entry means the download step misbehaved rather than that a
    release carried something unexpected. This pins the scanner's behaviour:
    only regular files are observed, and the declared set is what is compared.
    """
    staging = tmp_path / "dist"
    _stage(staging)
    (staging / "nested").mkdir()
    (staging / "link").symlink_to(staging / ARCHIVE)
    declaration = _declaration(tmp_path / "declared.txt", [ARCHIVE, SIDECAR])
    trusted = _write_record(
        tmp_path / "trusted.json",
        # Only the two declared files are covered: the record's job here is to
        # agree with the declaration, so the undeclared package must be absent
        # from it rather than supplied and then ignored.
        release_digest_record.DigestRecord.from_mapping({
            ARCHIVE: release_digest_files.digest_of_file(staging / ARCHIVE),
            SIDECAR: release_digest_files.digest_of_file(staging / SIDECAR),
        }),
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

    assert code == EXIT_OK, f"non-regular entries are not downloaded assets, got {code}"


def _fail_staging_directory(monkeypatch: pytest.MonkeyPatch) -> None:
    """Make creating the staging directory raise an output fault."""
    monkeypatch.setattr(
        verifier.tempfile,
        "TemporaryDirectory",
        mock.Mock(side_effect=OSError("simulated directory failure")),
    )


def _fail_staged_write(monkeypatch: pytest.MonkeyPatch) -> None:
    """Make writing the staged record raise an output fault."""
    monkeypatch.setattr(
        verifier.pathlib.Path,
        "write_text",
        mock.Mock(side_effect=PermissionError("simulated write failure")),
    )


@pytest.mark.parametrize(
    "install_fault",
    [
        pytest.param(_fail_staging_directory, id="staging-directory"),
        pytest.param(_fail_staged_write, id="staged-write"),
    ],
)
def test_a_failed_record_write_preserves_the_previous_record(
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    install_fault: cabc.Callable[[pytest.MonkeyPatch], None],
) -> None:
    """Report an output fault and leave an earlier record in place.

    Both ways of failing part-way through the staged write must behave alike:
    the run reports ``EXIT_OUTPUT_UNWRITABLE`` and the record from the previous
    run survives, which is what makes the exit status, not the file's presence,
    the thing consumers check. The case id names which step was made to fail.
    """
    staging = tmp_path / "dist"
    _stage(staging)
    declaration = _declaration(tmp_path / "declared.txt", [ARCHIVE, SIDECAR])
    trusted = _write_record(tmp_path / "trusted.json", _trusted(staging))
    destination = tmp_path / "verified.json"
    _ = destination.write_text("previous record\n", encoding="utf-8")
    install_fault(monkeypatch)

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

    assert code == EXIT_OUTPUT_UNWRITABLE, f"an output fault must be reported, {code}"
    assert destination.read_text(encoding="utf-8") == "previous record\n", (
        "a failed staged write must not change the earlier record"
    )
    assert "unwritable" in capsys.readouterr().err, (
        "the diagnostic must report the record as unwritable"
    )


def test_a_cleanup_failure_after_replacement_reports_unwritable(
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Report an output fault even though the new record was installed.

    The replacement completes before the staging directory is removed, so a
    cleanup fault leaves the new record in place while the run reports failure.
    Consumers must therefore check the exit status, not merely whether the
    output exists, or they would use a record whose run did not succeed.
    """
    staging = tmp_path / "dist"
    _stage(staging)
    declaration = _declaration(tmp_path / "declared.txt", [ARCHIVE, SIDECAR])
    trusted = _write_record(tmp_path / "trusted.json", _trusted(staging))
    destination = tmp_path / "verified.json"
    _ = destination.write_text("previous record\n", encoding="utf-8")

    class CleanupFails(tempfile.TemporaryDirectory):
        """Clean up normally, then report the simulated fault.

        Subclassing keeps the real directory's creation and entry, so only the
        cleanup the test is exercising differs.
        """

        def __exit__(
            self,
            exc: type[BaseException] | None,
            value: BaseException | None,
            tb: types.TracebackType | None,
        ) -> None:
            """Remove the real directory, then raise the simulated fault."""
            super().__exit__(exc, value, tb)
            raise _CleanupFaultError

    monkeypatch.setattr(verifier.tempfile, "TemporaryDirectory", CleanupFails)

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

    assert code == EXIT_OUTPUT_UNWRITABLE, f"an output fault must be reported, {code}"
    record = json.loads(destination.read_text(encoding="utf-8"))
    assert sorted(record["assets"]) == [ARCHIVE, SIDECAR], (
        "the replacement happens before cleanup, so the new record is installed"
    )
    assert "unwritable" in capsys.readouterr().err, (
        "the cleanup fault must still be reported as an output fault"
    )


def test_a_successful_run_replaces_an_existing_record(
    tmp_path: pathlib.Path,
) -> None:
    """Replace an earlier record completely rather than leaving it in place."""
    staging = tmp_path / "dist"
    _stage(staging)
    declaration = _declaration(tmp_path / "declared.txt", [ARCHIVE, SIDECAR])
    trusted = _write_record(tmp_path / "trusted.json", _trusted(staging))
    destination = tmp_path / "verified.json"
    _ = destination.write_text("previous record\n", encoding="utf-8")

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

    assert code == EXIT_OK, code
    record = json.loads(destination.read_text(encoding="utf-8"))
    assert sorted(record["assets"]) == [ARCHIVE, SIDECAR], (
        "a successful run must install the verified record"
    )
