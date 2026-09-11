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


def _run_archive_validator(
    archive_directory: pathlib.Path, output_directory: pathlib.Path
) -> int:
    """Run the composition command over one archive and output directory pair."""
    return _module().main([
        "--archive-dir",
        str(archive_directory),
        "--output-dir",
        str(output_directory),
    ])


def _helper() -> types.ModuleType:
    """Load the ZIP archive helper through an explicit module seam."""
    path = SCRIPT_DIRECTORY / "coverage_artifact_archive.py"
    specification = importlib.util.spec_from_file_location(
        "coverage_artifact_archive_test", path
    )
    assert specification is not None, "archive helper must be importable"
    assert specification.loader is not None, "archive helper must have a loader"
    module = importlib.util.module_from_spec(specification)
    sys.modules[specification.name] = module
    specification.loader.exec_module(module)
    return module


def _accept_text(text: str) -> None:
    """Accept decoded member text so a crafted contract reaches its path check."""
    del text


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

    _assert_contract(_run_archive_validator(archive_directory, output_directory) == 0)

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

    _assert_contract(_run_archive_validator(archive_directory, output_directory) == 1)

    captured = capsys.readouterr()
    _assert_contract(not captured.out)
    _assert_contract(captured.err == f"error: {expected_message}\n")
    _assert_contract(not output_directory.exists())


def test_archive_validation_rejects_an_oversized_member_before_writing(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Enforce the cumulative uncompressed ZIP-size bound before materializing."""
    archive_directory = tmp_path / "archive"
    _write_archive(
        archive_directory,
        [("lcov.info", b"x" * (16 * 1024 * 1024 + 1))],
    )
    output_directory = tmp_path / "validated"

    _assert_contract(_run_archive_validator(archive_directory, output_directory) == 1)

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

    _assert_contract(_run_archive_validator(archive_directory, output_directory) == 1)

    captured = capsys.readouterr()
    _assert_contract(not captured.out)
    _assert_contract(captured.err.startswith("error: artefact must contain only"))
    _assert_contract(not output_directory.exists())


def test_archive_validation_rejects_a_non_zip_archive(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Reject a sole outer file that is not a ZIP archive at all."""
    archive_directory = tmp_path / "archive"
    archive_directory.mkdir()
    (archive_directory / "artifact").write_bytes(b"not a ZIP archive")
    output_directory = tmp_path / "validated"

    _assert_contract(_run_archive_validator(archive_directory, output_directory) == 1)

    captured = capsys.readouterr()
    _assert_contract(not captured.out)
    _assert_contract(captured.err == "error: coverage artefact is not a ZIP archive\n")
    _assert_contract(not output_directory.exists())


def test_archive_validation_rejects_a_non_utf8_member(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Reject member bytes that are not decodable UTF-8 text."""
    archive_directory = tmp_path / "archive"
    _write_archive(archive_directory, [("lcov.info", b"\xff\xfelcov")])
    output_directory = tmp_path / "validated"

    _assert_contract(_run_archive_validator(archive_directory, output_directory) == 1)

    captured = capsys.readouterr()
    _assert_contract(not captured.out)
    _assert_contract(captured.err == "error: coverage report is not UTF-8 text\n")
    _assert_contract(not output_directory.exists())


@pytest.mark.parametrize(
    ("content", "expected_message"),
    [
        pytest.param(
            b"TN:\nSF:src/lib.rs\nDA:1,1\nbogus:record\nend_of_record\n",
            "invalid LCOV record at line 4",
            id="unrecognized-record",
        ),
        pytest.param(
            b"TN:\nSF:src/lib.rs\nDA:1,1\nend_of_record\nLF:1\n",
            "coverage report must end with end_of_record",
            id="missing-terminator",
        ),
        pytest.param(
            b"TN:\nSF:src/lib.rs\nDA:1,1\n",
            "coverage report is missing required end_of_record record",
            id="missing-required-record",
        ),
    ],
)
def test_archive_validation_rejects_malformed_lcov_text(
    tmp_path: pathlib.Path,
    capsys: pytest.CaptureFixture[str],
    content: bytes,
    expected_message: str,
) -> None:
    """Reject decodable member text that breaks the recognized LCOV contract."""
    archive_directory = tmp_path / "archive"
    _write_archive(archive_directory, [("lcov.info", content)])
    output_directory = tmp_path / "validated"

    _assert_contract(_run_archive_validator(archive_directory, output_directory) == 1)

    captured = capsys.readouterr()
    _assert_contract(not captured.out)
    _assert_contract(captured.err == f"error: {expected_message}\n")
    _assert_contract(not output_directory.exists())


@pytest.mark.parametrize("kind", ["populated", "symlink", "regular-file"])
def test_archive_validation_rejects_an_unsafe_output_destination(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str], kind: str
) -> None:
    """Refuse to write validated bytes into a linked, non-empty, or file target."""
    archive_directory = tmp_path / "archive"
    _write_archive(archive_directory, [("lcov.info", VALID_LCOV.encode())])
    output_directory = tmp_path / "validated"
    linked_target = tmp_path / "linked-target"
    match kind:
        case "populated":
            output_directory.mkdir()
            (output_directory / "existing").write_text("hostile", encoding="utf-8")
        case "symlink":
            linked_target.mkdir()
            output_directory.symlink_to(linked_target, target_is_directory=True)
        case _:
            output_directory.write_text("hostile", encoding="utf-8")

    _assert_contract(_run_archive_validator(archive_directory, output_directory) == 1)

    captured = capsys.readouterr()
    _assert_contract(not captured.out)
    _assert_contract(captured.err == "error: validated output directory is unsafe\n")
    _assert_contract(not (linked_target / "lcov.info").exists())


@pytest.mark.parametrize(
    ("members", "expected_issue"),
    [
        pytest.param(
            [("lcov.info", VALID_LCOV.encode()), ("extra", b"hostile")],
            "members",
            id="extra-member",
        ),
        pytest.param([("lcov.info", _symlink_member())], "type", id="symlink-member"),
        pytest.param(
            [("lcov.info", b"x" * (16 * 1024 * 1024 + 1))],
            "size",
            id="oversized-member",
        ),
    ],
)
def test_archive_validation_keeps_its_published_classifications(
    tmp_path: pathlib.Path,
    members: list[tuple[str, bytes | zipfile.ZipInfo]],
    expected_issue: str,
) -> None:
    """Keep every hostile metadata case mapped to its published issue value."""
    helper = _helper()
    archive_path = _write_archive(tmp_path / "archive", members)
    output_directory = tmp_path / "validated"

    with pytest.raises(helper.ArchiveValidationError) as error:
        helper.validate_and_materialize(
            archive_path,
            output_directory,
            helper.ArchiveContract("lcov.info", 16 * 1024 * 1024, _accept_text),
        )

    _assert_contract(error.value.issue == helper.ArchiveIssue(expected_issue))
    _assert_contract(not output_directory.exists())


def test_archive_validation_rejects_a_crafted_contract_member_path(
    tmp_path: pathlib.Path,
) -> None:
    """Reject a member name that is absolute or escapes its packed directory."""
    helper = _helper()
    archive_directory = tmp_path / "archive"
    archive_directory.mkdir()
    archive_path = archive_directory / "artifact"
    contract = helper.ArchiveContract("../lcov.info", 16 * 1024 * 1024, _accept_text)
    with zipfile.ZipFile(archive_path, "w") as archive:
        archive.writestr("../lcov.info", VALID_LCOV)
    output_directory = tmp_path / "validated"

    with pytest.raises(helper.ArchiveValidationError) as error:
        helper.validate_and_materialize(archive_path, output_directory, contract)

    _assert_contract(error.value.issue is helper.ArchiveIssue.PATH)
    _assert_contract(not output_directory.exists())
