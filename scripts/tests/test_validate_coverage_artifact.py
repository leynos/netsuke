"""Exercise hostile LCOV artefact validation in-process and as a command.

The trusted workflow consumes data uploaded by an untrusted pull-request run.
These cases prove that only a small, regular LCOV report can cross that
boundary; links, extra files, oversized input, and malformed text fail before
the secret-bearing CodeScene action starts.
"""

import dataclasses
import importlib.util
import os
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the CLI boundary is under test.
import sys
import typing as typ

import pytest
from conftest import SCRIPT_DIRECTORY

if typ.TYPE_CHECKING:
    import pathlib
    import types


VALID_LCOV = "TN:\nSF:src/lib.rs\nDA:1,1\nLF:1\nLH:1\nend_of_record\n"


@dataclasses.dataclass(frozen=True, slots=True)
class ArtefactCase:
    """Define one artefact shape and its expected validation issue."""

    name: str
    expected_code: int
    expected_issue: str | None


def _load_script() -> types.ModuleType:
    """Load the hyphenated validator module for its in-process seam."""
    spec = importlib.util.spec_from_file_location(
        "validate_coverage_artifact", SCRIPT_DIRECTORY / "validate-coverage-artifact.py"
    )
    assert spec is not None, "expected coverage validator import setup"
    assert spec.loader is not None, "expected coverage validator loader"
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def _write_missing_member(directory: pathlib.Path, case: ArtefactCase) -> None:
    """Create an empty artefact directory without its expected member."""


def _write_symlinked_directory(directory: pathlib.Path, case: ArtefactCase) -> None:
    """Replace the artefact directory itself with a symbolic link."""
    real_directory = directory.parent / f"{directory.name}-real"
    real_directory.mkdir()
    (real_directory / "lcov.info").write_text(VALID_LCOV, encoding="utf-8")
    directory.rmdir()
    directory.symlink_to(real_directory, target_is_directory=True)


def _write_non_directory(directory: pathlib.Path, case: ArtefactCase) -> None:
    """Replace the artefact directory with a regular file."""
    directory.rmdir()
    directory.write_text(VALID_LCOV, encoding="utf-8")


def _write_directory_member(directory: pathlib.Path, case: ArtefactCase) -> None:
    """Create the expected member as a directory instead of a file."""
    (directory / "lcov.info").mkdir()


def _write_symlink_member(directory: pathlib.Path, case: ArtefactCase) -> None:
    """Create a dangling symbolic-link coverage member."""
    target = directory / "outside-lcov.info"
    target.write_text(VALID_LCOV, encoding="utf-8")
    (directory / "lcov.info").symlink_to(target)
    target.unlink()


def _write_symlink_to_valid_external_file(
    directory: pathlib.Path, case: ArtefactCase
) -> None:
    """Link the coverage member at a valid file outside the artefact."""
    external_file = directory.parent / "external-lcov.info"
    external_file.write_text(VALID_LCOV, encoding="utf-8")
    (directory / "lcov.info").symlink_to(external_file)


def _write_oversized_member(directory: pathlib.Path, case: ArtefactCase) -> None:
    """Create a coverage member one byte beyond the configured size bound."""
    coverage_path = directory / "lcov.info"
    coverage_path.write_text(VALID_LCOV, encoding="utf-8")
    coverage_path.write_bytes(b"x" * (16 * 1024 * 1024 + 1))


def _write_non_utf8_member(directory: pathlib.Path, case: ArtefactCase) -> None:
    """Create a coverage member that is not UTF-8 text."""
    (directory / "lcov.info").write_bytes(b"\xff\xfe\x00")


def _write_default_member(directory: pathlib.Path, case: ArtefactCase) -> None:
    """Write a valid or malformed report, plus any extra hostile member."""
    (directory / "lcov.info").write_text(
        "unexpected data\n" if case.name == "malformed" else VALID_LCOV,
        encoding="utf-8",
    )
    if case.name == "extra-member":
        (directory / "unexpected.txt").write_text("hostile", encoding="utf-8")


ARTEFACT_WRITERS = {
    "missing-member": _write_missing_member,
    "symlinked-directory": _write_symlinked_directory,
    "non-directory": _write_non_directory,
    "directory-member": _write_directory_member,
    "symlink-member": _write_symlink_member,
    "symlink-to-valid-external-file": _write_symlink_to_valid_external_file,
    "oversized": _write_oversized_member,
    "non-utf8": _write_non_utf8_member,
}


def _write_case(directory: pathlib.Path, case: ArtefactCase) -> None:
    """Create the requested valid or hostile artefact fixture."""
    writer = ARTEFACT_WRITERS.get(case.name, _write_default_member)
    writer(directory, case)


def _expected_detail(
    script: types.ModuleType, directory: pathlib.Path, case: ArtefactCase
) -> object | None:
    """Return the exact controlled diagnostic detail for one fixture."""
    match case.name:
        case "missing-member":
            return []
        case "non-directory":
            return directory
        case "oversized":
            return (script.MAXIMUM_COVERAGE_BYTES + 1, script.MAXIMUM_COVERAGE_BYTES)
        case "malformed":
            return 1
        case _:
            return None


def _validate_case(
    script: types.ModuleType, directory: pathlib.Path, case: ArtefactCase
) -> object | None:
    """Validate one fixture and return its captured error when rejected."""
    if case.expected_issue is None:
        script.validate(directory)
        return None
    with pytest.raises(script.ValidationError) as captured:
        script.validate(directory)
    error = captured.value
    assert error.issue is getattr(script.ValidationIssue, case.expected_issue), (
        f"{case.name} must raise {case.expected_issue}, got {error.issue!r}"
    )
    assert error.detail == _expected_detail(script, directory, case), (
        f"{case.name} must preserve its diagnostic detail"
    )
    return error


@pytest.mark.parametrize(
    ("text", "expected_issue", "expected_detail"),
    [
        pytest.param("", "EMPTY_REPORT", None, id="empty"),
        pytest.param(
            "TN:\nnot-an-lcov-record\nSF:src/lib.rs\nDA:1,1\nend_of_record\n",
            "INVALID_RECORD",
            2,
            id="invalid-line",
        ),
        pytest.param(
            "TN:\nDA:1,1\nend_of_record\n", "MISSING_RECORD", "SF:", id="missing-sf"
        ),
        pytest.param(
            "TN:\nSF:src/lib.rs\nend_of_record\n",
            "MISSING_RECORD",
            "DA:",
            id="missing-da",
        ),
        pytest.param(
            "TN:\nSF:src/lib.rs\nDA:1,1\n",
            "MISSING_RECORD",
            "end_of_record",
            id="missing-end",
        ),
        pytest.param(
            "TN:\nSF:src/lib.rs\nDA:1,1\nend_of_record\nTN:\n",
            "MISSING_TERMINATOR",
            None,
            id="end-before-final-line",
        ),
        pytest.param(
            "TN:\nTN:SF:fake\nTN:DA:1,1\nend_of_record\n",
            "MISSING_RECORD",
            "SF:",
            id="fake-embedded-sf-record",
        ),
        pytest.param(
            "TN:\nSF:src/lib.rs\nTN:DA:1,1\nend_of_record\n",
            "MISSING_RECORD",
            "DA:",
            id="fake-embedded-da-record",
        ),
    ],
)
def test_validate_lcov_text_preserves_diagnostic_order(
    text: str, expected_issue: str, expected_detail: object | None
) -> None:
    """Return the first diagnostic defined by the hostile-LCOV contract."""
    script = _load_script()

    with pytest.raises(script.ValidationError) as captured:
        script._validate_lcov_text(text)

    error = captured.value
    assert error.issue is getattr(script.ValidationIssue, expected_issue), (
        f"expected issue {expected_issue!r}, got {error.issue!r}"
    )
    assert error.detail == expected_detail, (
        f"expected detail {expected_detail!r}, got {error.detail!r}"
    )


@pytest.mark.parametrize(
    "case",
    [
        pytest.param(ArtefactCase("valid", 0, None), id="valid"),
        pytest.param(
            ArtefactCase("missing-member", 1, "UNEXPECTED_MEMBERS"),
            id="missing-member",
        ),
        pytest.param(
            ArtefactCase("extra-member", 1, "UNEXPECTED_MEMBERS"),
            id="extra-member",
        ),
        pytest.param(
            ArtefactCase("symlink-member", 1, "SYMLINK_MEMBER"),
            id="symlink-member",
        ),
        pytest.param(
            ArtefactCase("symlink-to-valid-external-file", 1, "SYMLINK_MEMBER"),
            id="symlink-to-valid-external-file",
        ),
        pytest.param(
            ArtefactCase("directory-member", 1, "NON_REGULAR_MEMBER"),
            id="directory-member",
        ),
        pytest.param(
            ArtefactCase("symlinked-directory", 1, "SYMLINK_DIRECTORY"),
            id="symlinked-directory",
        ),
        pytest.param(
            ArtefactCase("non-directory", 1, "NON_DIRECTORY"),
            id="non-directory",
        ),
        pytest.param(ArtefactCase("oversized", 1, "OVERSIZED_REPORT"), id="oversized"),
        pytest.param(ArtefactCase("non-utf8", 1, "NON_UTF8_REPORT"), id="non-utf8"),
        pytest.param(ArtefactCase("malformed", 1, "INVALID_RECORD"), id="malformed"),
    ],
)
def test_validate_filesystem_boundaries_in_process(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str], case: ArtefactCase
) -> None:
    """Preserve issue, detail, status, and streams for every artefact shape."""
    _write_case(tmp_path, case)
    script = _load_script()
    error = _validate_case(script, tmp_path, case)

    assert script.main(["--artifact-dir", str(tmp_path)]) == case.expected_code, (
        f"{case.name} must preserve the documented in-process exit code"
    )
    captured = capsys.readouterr()
    if error is None:
        assert captured.out == "ok: validated hostile LCOV artefact\n", (
            "a valid artefact must retain its success message"
        )
        assert not captured.err, "a valid artefact must not write stderr"
    else:
        assert not captured.out, "a rejected artefact must not write stdout"
        assert captured.err == f"error: {error}\n", (
            "a rejected artefact must retain its formatted diagnostic"
        )


@pytest.mark.parametrize(
    "case",
    [
        pytest.param(ArtefactCase("valid", 0, None), id="valid"),
        pytest.param(
            ArtefactCase("missing-member", 1, "UNEXPECTED_MEMBERS"),
            id="missing-member",
        ),
        pytest.param(
            ArtefactCase("extra-member", 1, "UNEXPECTED_MEMBERS"),
            id="extra-member",
        ),
        pytest.param(
            ArtefactCase("symlink-member", 1, "SYMLINK_MEMBER"),
            id="symlink-member",
        ),
        pytest.param(
            ArtefactCase("symlink-to-valid-external-file", 1, "SYMLINK_MEMBER"),
            id="symlink-to-valid-external-file",
        ),
        pytest.param(
            ArtefactCase("directory-member", 1, "NON_REGULAR_MEMBER"),
            id="directory-member",
        ),
        pytest.param(
            ArtefactCase("symlinked-directory", 1, "SYMLINK_DIRECTORY"),
            id="symlinked-directory",
        ),
        pytest.param(
            ArtefactCase("non-directory", 1, "NON_DIRECTORY"),
            id="non-directory",
        ),
        pytest.param(ArtefactCase("oversized", 1, "OVERSIZED_REPORT"), id="oversized"),
        pytest.param(ArtefactCase("non-utf8", 1, "NON_UTF8_REPORT"), id="non-utf8"),
        pytest.param(ArtefactCase("malformed", 1, "INVALID_RECORD"), id="malformed"),
    ],
)
def test_validator_cli_preserves_filesystem_boundaries(
    tmp_path: pathlib.Path, case: ArtefactCase
) -> None:
    """Preserve every filesystem boundary through the real command line."""
    _write_case(tmp_path, case)
    script = _load_script()
    error = _validate_case(script, tmp_path, case)
    result = subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - shell is False.
        [
            sys.executable,
            str(SCRIPT_DIRECTORY / "validate-coverage-artifact.py"),
            "--artifact-dir",
            str(tmp_path),
        ],
        capture_output=True,
        check=False,
        text=True,
    )

    assert result.returncode == case.expected_code, result.stderr
    if error is None:
        assert result.stdout == "ok: validated hostile LCOV artefact\n", (
            "the CLI must retain its success message"
        )
        assert not result.stderr, "the successful CLI must not write stderr"
    else:
        assert not result.stdout, "the rejected CLI must not write stdout"
        assert result.stderr == f"error: {error}\n", (
            "the rejected CLI must retain its formatted diagnostic"
        )


def test_validator_cli_never_executes_artefact_content(tmp_path: pathlib.Path) -> None:
    """Reject hostile shell and report text without executing either payload."""
    case = ArtefactCase("malformed", 1, "INVALID_RECORD")
    _write_case(tmp_path, case)
    shell_marker = tmp_path / "shell-executed"
    artefact_marker = tmp_path / "artefact-executed"
    hostile_shell = tmp_path.parent / "hostile-bash-env"
    hostile_shell.write_text(f"touch {shell_marker}\n", encoding="utf-8")
    (tmp_path / "lcov.info").write_text(
        f"$(touch {artefact_marker})\n", encoding="utf-8"
    )
    environment = os.environ | {"BASH_ENV": str(hostile_shell)}

    result = subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - shell is False.
        [
            sys.executable,
            str(SCRIPT_DIRECTORY / "validate-coverage-artifact.py"),
            "--artifact-dir",
            str(tmp_path),
        ],
        capture_output=True,
        check=False,
        env=environment,
        text=True,
    )

    assert result.returncode == 1, "hostile content must remain a validation failure"
    assert not result.stdout, "a rejected hostile report must not write stdout"
    assert result.stderr.startswith("error: invalid LCOV record at line 1\n"), (
        "hostile content must be reported as an invalid record"
    )
    assert not shell_marker.exists(), "the validator must not source BASH_ENV"
    assert not artefact_marker.exists(), "the validator must not execute artefact text"
