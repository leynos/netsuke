"""Exercise ZIP inspection before trusted coverage materialization.

The download action leaves its untrusted artifact compressed. These cases prove
that the archive validator rejects hostile metadata before it writes a report
for the secret-bearing CodeScene action to consume.
"""

import importlib.util
import stat
import sys
import typing as typ
import zipfile

import pytest
from conftest import SCRIPT_DIRECTORY

if typ.TYPE_CHECKING:
    import pathlib
    import types


VALID_LCOV = "TN:\nSF:src/lib.rs\nDA:1,1\nLF:1\nLH:1\nend_of_record\n"


def _assert_contract(condition: object) -> None:
    """Assert one archive-boundary contract with a consistent diagnostic."""
    assert condition, "archive validation contract failed"


def _module() -> types.ModuleType:
    """Load the archive validator through its production command seam."""
    path = SCRIPT_DIRECTORY / "validate_coverage_archive.py"
    specification = importlib.util.spec_from_file_location(
        "validate_coverage_archive_test", path
    )
    assert specification is not None, "archive validator must be importable"
    assert specification.loader is not None, "archive validator must have a loader"
    module = importlib.util.module_from_spec(specification)
    sys.modules[specification.name] = module
    specification.loader.exec_module(module)
    return module


def _write_archive(
    directory: pathlib.Path, members: list[tuple[str, bytes | zipfile.ZipInfo]]
) -> pathlib.Path:
    """Write one raw archive whose member metadata is controlled by each test."""
    directory.mkdir()
    archive_path = directory / "artifact"
    with zipfile.ZipFile(archive_path, "w") as archive:
        for name, content in members:
            if isinstance(content, zipfile.ZipInfo):
                archive.writestr(content, VALID_LCOV)
            else:
                archive.writestr(name, content)
    return archive_path


def _symlink_member() -> zipfile.ZipInfo:
    """Return ZIP metadata for a symbolic-link member without a filesystem link."""
    member = zipfile.ZipInfo("lcov.info")
    member.external_attr = (stat.S_IFLNK | 0o777) << 16
    return member


def test_archive_validation_materializes_only_a_valid_lcov_member(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Write the sole checked LCOV report through the controlled output path."""
    archive_directory = tmp_path / "archive"
    _write_archive(archive_directory, [("lcov.info", VALID_LCOV.encode())])
    output_directory = tmp_path / "validated"

    _assert_contract(
        _module().main([
            "--archive-dir",
            str(archive_directory),
            "--output-dir",
            str(output_directory),
        ])
        == 0
    )

    captured = capsys.readouterr()
    _assert_contract(
        captured.out == "ok: validated and materialized hostile LCOV artefact\n"
    )
    _assert_contract(not captured.err)
    _assert_contract(
        (output_directory / "lcov.info").read_text(encoding="utf-8") == VALID_LCOV
    )


@pytest.mark.parametrize(
    ("members", "expected_message"),
    [
        pytest.param(
            [("lcov.info", VALID_LCOV.encode()), ("extra", b"hostile")],
            "archive must contain exactly lcov.info",
            id="extra-member",
        ),
        pytest.param(
            [("../lcov.info", VALID_LCOV.encode())],
            "archive must contain exactly lcov.info",
            id="escaped-member-path",
        ),
        pytest.param(
            [
                (
                    "lcov.info",
                    _symlink_member(),
                )
            ],
            "archive member must be a regular non-link file",
            id="symlink-member",
        ),
    ],
)
def test_archive_validation_rejects_hostile_metadata_before_materialization(
    tmp_path: pathlib.Path,
    capsys: pytest.CaptureFixture[str],
    members: list[tuple[str, bytes | zipfile.ZipInfo]],
    expected_message: str,
) -> None:
    """Reject unsafe ZIP members without creating trusted output data."""
    archive_directory = tmp_path / "archive"
    _write_archive(archive_directory, members)
    output_directory = tmp_path / "validated"

    _assert_contract(
        _module().main([
            "--archive-dir",
            str(archive_directory),
            "--output-dir",
            str(output_directory),
        ])
        == 1
    )

    captured = capsys.readouterr()
    _assert_contract(not captured.out)
    _assert_contract(captured.err == f"error: {expected_message}\n")
    _assert_contract(not output_directory.exists())


def test_archive_validation_rejects_an_oversized_member_before_writing(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Enforce the cumulative uncompressed ZIP-size bound before materializing."""
    module = _module()
    archive_directory = tmp_path / "archive"
    _write_archive(
        archive_directory,
        [("lcov.info", b"x" * (16 * 1024 * 1024 + 1))],
    )
    output_directory = tmp_path / "validated"

    _assert_contract(
        module.main([
            "--archive-dir",
            str(archive_directory),
            "--output-dir",
            str(output_directory),
        ])
        == 1
    )

    captured = capsys.readouterr()
    _assert_contract(not captured.out)
    _assert_contract(captured.err == "error: archive uncompressed size exceeds limit\n")
    _assert_contract(not output_directory.exists())


def test_archive_validation_rejects_multiple_outer_files_before_zip_inspection(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Reject a raw artifact directory that does not contain exactly one file."""
    archive_directory = tmp_path / "archive"
    _write_archive(archive_directory, [("lcov.info", VALID_LCOV.encode())])
    (archive_directory / "unexpected").write_text("hostile", encoding="utf-8")
    output_directory = tmp_path / "validated"

    _assert_contract(
        _module().main([
            "--archive-dir",
            str(archive_directory),
            "--output-dir",
            str(output_directory),
        ])
        == 1
    )

    captured = capsys.readouterr()
    _assert_contract(not captured.out)
    _assert_contract(captured.err.startswith("error: artefact must contain only"))
    _assert_contract(not output_directory.exists())
