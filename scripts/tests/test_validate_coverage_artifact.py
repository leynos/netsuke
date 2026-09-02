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


def _write_case(directory: pathlib.Path, case: ArtefactCase) -> None:
    """Create the requested valid or hostile artefact fixture."""
    if case.name == "missing-member":
        return
    coverage_path = directory / "lcov.info"
    if case.name == "symlink-member":
        target = directory / "outside-lcov.info"
        target.write_text(VALID_LCOV, encoding="utf-8")
        coverage_path.symlink_to(target)
        target.unlink()
        return
    if case.name == "oversized":
        coverage_path.write_text(VALID_LCOV, encoding="utf-8")
        coverage_path.write_bytes(b"x" * (16 * 1024 * 1024 + 1))
        return
    if case.name == "non-utf8":
        coverage_path.write_bytes(b"\xff\xfe\x00")
        return
    coverage_path.write_text(
        "unexpected data\n" if case.name == "malformed" else VALID_LCOV,
        encoding="utf-8",
    )
    if case.name == "extra-member":
        (directory / "unexpected.txt").write_text("hostile", encoding="utf-8")


@pytest.mark.parametrize(
    "case",
    [
        pytest.param(ArtefactCase("valid", 0), id="valid"),
        pytest.param(ArtefactCase("missing-member", 1), id="missing-member"),
        pytest.param(ArtefactCase("extra-member", 1), id="extra-member"),
        pytest.param(ArtefactCase("symlink-member", 1), id="symlink-member"),
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
