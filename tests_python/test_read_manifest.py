"""Tests for the read_manifest helper script."""

from __future__ import annotations

import dataclasses
import importlib.util
import os
import subprocess
import sys
import types
import typing as typ
from contextlib import ExitStack, contextmanager, redirect_stderr, redirect_stdout
from io import StringIO
from pathlib import Path
from textwrap import dedent
from unittest import mock

import pytest

REPO_ROOT = Path(__file__).resolve().parent.parent
SCRIPT_PATH = REPO_ROOT / ".github" / "workflows" / "scripts" / "read_manifest.py"


@dataclasses.dataclass(slots=True)
class CLIResult:
    """Result container returned by :func:`ReadManifestTests._invoke_cli`."""

    exit_code: int
    stdout: str
    stderr: str


@contextmanager
def change_directory(path: Path) -> typ.Iterator[None]:
    """Temporarily change the working directory for the current process."""
    original = Path.cwd()
    os.chdir(path)
    try:
        yield
    finally:
        os.chdir(original)


def load_script_module() -> types.ModuleType:
    """Import the read_manifest script as a module for reuse in tests."""
    spec = importlib.util.spec_from_file_location("read_manifest", SCRIPT_PATH)
    module = importlib.util.module_from_spec(spec)  # type: ignore[arg-type]
    assert spec is not None
    assert spec.loader is not None
    spec.loader.exec_module(module)  # type: ignore[assignment]
    assert isinstance(module, types.ModuleType)
    return module


@dataclasses.dataclass(slots=True)
class ReadManifestTests:
    """Helpers that exercise the manifest-reading CLI in different scenarios."""

    module: types.ModuleType
    temp_path: Path

    def _write_manifest(self, content: str) -> Path:
        """Write ``content`` to ``Cargo.toml`` in the temporary directory."""
        manifest = self.temp_path / "Cargo.toml"
        manifest.write_text(dedent(content), encoding="utf-8")
        return manifest

    def _invoke_cli(
        self,
        *args: str,
        env: dict[str, str] | None = None,
        cwd: Path | None = None,
    ) -> CLIResult:
        """Execute the CLI and capture its exit code and output streams."""
        stdout = StringIO()
        stderr = StringIO()
        with ExitStack() as stack:
            stack.enter_context(
                mock.patch.object(sys, "argv", [str(SCRIPT_PATH), *args])
            )
            if env:
                stack.enter_context(mock.patch.dict(os.environ, env, clear=False))
            if cwd:
                stack.enter_context(change_directory(cwd))
            stack.enter_context(redirect_stdout(stdout))
            stack.enter_context(redirect_stderr(stderr))
            exit_code = self.module.main()
        return CLIResult(
            exit_code=exit_code,
            stdout=stdout.getvalue(),
            stderr=stderr.getvalue(),
        )

    def _assert_manifest_error(
        self,
        manifest_path: Path,
        expected_stderr_fragment: str | None = None,
    ) -> None:
        """Assert that invoking the CLI fails for ``manifest_path``."""
        result = subprocess.run(  # noqa: S603 - executed with trusted inputs in tests
            [
                sys.executable,
                str(SCRIPT_PATH),
                "name",
                "--manifest-path",
                str(manifest_path),
            ],
            check=False,
            capture_output=True,
            text=True,
        )
        assert result.returncode != 0
        if expected_stderr_fragment is not None:
            assert expected_stderr_fragment in result.stderr
        else:
            assert result.stderr
        assert result.stdout == ""

    def _assert_successful_field_read(
        self,
        manifest_content: str,
        field: str,
        expected_value: str,
        *,
        cli_args: tuple[str, ...] | None = None,
        env: dict[str, str] | None = None,
        cwd: Path | None = None,
    ) -> None:
        """Assert that the CLI prints ``expected_value`` for ``field``."""
        manifest = self._write_manifest(manifest_content)
        args = cli_args or (field, "--manifest-path", str(manifest))
        result = self._invoke_cli(*args, env=env, cwd=cwd)
        assert result.exit_code == 0
        assert result.stdout == expected_value
        assert result.stderr == ""


@pytest.fixture(scope="module")
def read_manifest_module() -> types.ModuleType:
    """Load the read_manifest script once for all tests."""
    return load_script_module()


@pytest.fixture
def read_manifest_tests(
    read_manifest_module: types.ModuleType,
    tmp_path: Path,
) -> ReadManifestTests:
    """Provide helpers that operate within a temporary working directory."""
    return ReadManifestTests(module=read_manifest_module, temp_path=tmp_path)


def test_get_field_returns_name(read_manifest_module: types.ModuleType) -> None:
    """It returns the package name from the manifest."""
    manifest = {"package": {"name": "netsuke", "version": "1.2.3"}}
    assert read_manifest_module.get_field(manifest, "name") == "netsuke"


def test_get_field_returns_version(read_manifest_module: types.ModuleType) -> None:
    """It returns the package version from the manifest."""
    manifest = {"package": {"name": "netsuke", "version": "1.2.3"}}
    assert read_manifest_module.get_field(manifest, "version") == "1.2.3"


def test_get_field_raises_when_missing(read_manifest_module: types.ModuleType) -> None:
    """It raises when the requested field is absent."""
    manifest = {"package": {"name": "netsuke"}}
    with pytest.raises(KeyError):
        read_manifest_module.get_field(manifest, "version")


def test_get_field_rejects_non_string_values(
    read_manifest_module: types.ModuleType,
) -> None:
    """It rejects non-string manifest entries."""
    manifest = {
        "package": {
            "name": "netsuke",
            "version": 123,
            "authors": ["alice", "bob"],
            "metadata": {"license": "MIT"},
        }
    }
    with pytest.raises(KeyError):
        read_manifest_module.get_field(manifest, "version")
    with pytest.raises(KeyError):
        read_manifest_module.get_field(manifest, "authors")
    with pytest.raises(KeyError):
        read_manifest_module.get_field(manifest, "metadata")


def test_main_reads_manifest_path_argument(
    read_manifest_tests: ReadManifestTests,
) -> None:
    """It reads manifests from the path provided via CLI arguments."""
    read_manifest_tests._assert_successful_field_read(
        """
        [package]
        name = "netsuke"
        version = "1.2.3"
        """,
        field="name",
        expected_value="netsuke",
    )


def test_main_prefers_environment_manifest_path(
    read_manifest_tests: ReadManifestTests,
) -> None:
    """It prefers the manifest path supplied via environment variable."""
    manifest = read_manifest_tests._write_manifest(
        """
        [package]
        name = "netsuke"
        version = "1.2.3"
        """
    )
    env = {"CARGO_TOML_PATH": str(manifest)}
    result = read_manifest_tests._invoke_cli(
        "version",
        env=env,
        cwd=read_manifest_tests.temp_path,
    )
    assert result.exit_code == 0
    assert result.stdout == "1.2.3"
    assert result.stderr == ""


def test_main_reports_missing_manifest(
    read_manifest_tests: ReadManifestTests,
) -> None:
    """It surfaces errors when the manifest file does not exist."""
    missing = read_manifest_tests.temp_path / "missing.toml"
    read_manifest_tests._assert_manifest_error(missing, "does not exist")


def test_main_reports_invalid_toml(read_manifest_tests: ReadManifestTests) -> None:
    """It surfaces errors for invalid TOML content."""
    manifest = read_manifest_tests._write_manifest("not = [valid")
    read_manifest_tests._assert_manifest_error(manifest)


def test_main_reports_valid_toml_with_unexpected_structure(
    read_manifest_tests: ReadManifestTests,
) -> None:
    """It reports a descriptive error when required sections are missing."""
    manifest = read_manifest_tests._write_manifest(
        """
        [unexpected_section]
        foo = "bar"
        """
    )
    read_manifest_tests._assert_manifest_error(manifest, "missing")
