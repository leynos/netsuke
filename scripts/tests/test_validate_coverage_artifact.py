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
    """Define one artefact shape and its expected validator exit code."""

    name: str
    expected_code: int


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
        pytest.param(ArtefactCase("valid", 0), id="valid"),
        pytest.param(ArtefactCase("missing-member", 1), id="missing-member"),
        pytest.param(ArtefactCase("extra-member", 1), id="extra-member"),
        pytest.param(ArtefactCase("symlink-member", 1), id="symlink-member"),
        pytest.param(
            ArtefactCase("symlink-to-valid-external-file", 1),
            id="symlink-to-valid-external-file",
        ),
        pytest.param(ArtefactCase("directory-member", 1), id="directory-member"),
        pytest.param(ArtefactCase("symlinked-directory", 1), id="symlinked-directory"),
        pytest.param(ArtefactCase("non-directory", 1), id="non-directory"),
        pytest.param(ArtefactCase("oversized", 1), id="oversized"),
        pytest.param(ArtefactCase("non-utf8", 1), id="non-utf8"),
        pytest.param(ArtefactCase("malformed", 1), id="malformed"),
    ],
)
def test_validate_exit_codes_in_process(
    tmp_path: pathlib.Path, case: ArtefactCase
) -> None:
    """Return the documented status for every supported artefact shape."""
    _write_case(tmp_path, case)
    script = _load_script()

    assert script.main(["--artifact-dir", str(tmp_path)]) == case.expected_code, (
        f"{case.name} must return {case.expected_code}"
    )


@pytest.mark.parametrize(
    "case",
    [
        pytest.param(ArtefactCase("valid", 0), id="valid"),
        pytest.param(ArtefactCase("malformed", 1), id="malformed"),
        pytest.param(ArtefactCase("symlink-member", 1), id="symlink-member"),
    ],
)
def test_validator_cli_never_executes_artefact_content(
    tmp_path: pathlib.Path, case: ArtefactCase
) -> None:
    """Preserve the same boundary when the validator runs as a child process."""
    _write_case(tmp_path, case)
    marker = tmp_path / "executed"
    environment = os.environ | {"BASH_ENV": str(marker)}
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

    assert result.returncode == case.expected_code, result.stderr
    assert not marker.exists(), "the validator must not source hostile shell state"
