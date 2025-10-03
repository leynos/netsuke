"""Tests for the read_manifest helper script."""

from __future__ import annotations

from contextlib import ExitStack, contextmanager, redirect_stderr, redirect_stdout
from dataclasses import dataclass
import importlib.util
from io import StringIO
import os
import sys
from pathlib import Path
from tempfile import TemporaryDirectory
from textwrap import dedent
from typing import Any

import unittest
from unittest.mock import patch


REPO_ROOT = Path(__file__).resolve().parent.parent
SCRIPT_PATH = REPO_ROOT / ".github" / "workflows" / "scripts" / "read_manifest.py"


@dataclass(slots=True)
class CLIResult:
    """Result container returned by :func:`ReadManifestTests._invoke_cli`."""

    exit_code: int
    stdout: str
    stderr: str


@contextmanager
def change_directory(path: Path) -> Any:
    """Temporarily change the working directory for the current process."""

    original = Path.cwd()
    os.chdir(path)
    try:
        yield
    finally:
        os.chdir(original)


def load_script_module() -> Any:
    spec = importlib.util.spec_from_file_location("read_manifest", SCRIPT_PATH)
    module = importlib.util.module_from_spec(spec)  # type: ignore[arg-type]
    assert spec and spec.loader
    spec.loader.exec_module(module)  # type: ignore[assignment]
    return module


class ReadManifestTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.module = load_script_module()

    def setUp(self) -> None:
        self.tempdir = TemporaryDirectory()
        self.temp_path = Path(self.tempdir.name)

    def tearDown(self) -> None:
        self.tempdir.cleanup()

    def _write_manifest(self, content: str) -> Path:
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
            stack.enter_context(patch.object(sys, "argv", [str(SCRIPT_PATH), *args]))
            if env:
                stack.enter_context(patch.dict(os.environ, env, clear=False))
            if cwd:
                stack.enter_context(change_directory(cwd))
            stack.enter_context(redirect_stdout(stdout))
            stack.enter_context(redirect_stderr(stderr))
            exit_code = self.module.main()
        return CLIResult(exit_code=exit_code, stdout=stdout.getvalue(), stderr=stderr.getvalue())

    def test_get_field_returns_name(self) -> None:
        manifest = {"package": {"name": "netsuke", "version": "1.2.3"}}
        self.assertEqual(self.module.get_field(manifest, "name"), "netsuke")

    def test_get_field_returns_version(self) -> None:
        manifest = {"package": {"name": "netsuke", "version": "1.2.3"}}
        self.assertEqual(self.module.get_field(manifest, "version"), "1.2.3")

    def test_get_field_raises_when_missing(self) -> None:
        manifest = {"package": {"name": "netsuke"}}
        with self.assertRaises(KeyError):
            self.module.get_field(manifest, "version")

    def test_get_field_rejects_non_string_values(self) -> None:
        manifest = {
            "package": {
                "name": "netsuke",
                "version": 123,
                "authors": ["alice", "bob"],
                "metadata": {"license": "MIT"},
            }
        }
        with self.assertRaises(KeyError):
            self.module.get_field(manifest, "version")
        with self.assertRaises(KeyError):
            self.module.get_field(manifest, "authors")
        with self.assertRaises(KeyError):
            self.module.get_field(manifest, "metadata")

    def test_main_reads_manifest_path_argument(self) -> None:
        manifest = self._write_manifest(
            """
            [package]
            name = "netsuke"
            version = "1.2.3"
            """
        )
        result = self._invoke_cli("name", "--manifest-path", str(manifest))
        self.assertEqual(result.exit_code, 0)
        self.assertEqual(result.stdout, "netsuke")
        self.assertEqual(result.stderr, "")

    def test_main_prefers_environment_manifest_path(self) -> None:
        manifest = self._write_manifest(
            """
            [package]
            name = "netsuke"
            version = "1.2.3"
            """
        )
        env = {"CARGO_TOML_PATH": str(manifest)}
        result = self._invoke_cli("version", env=env, cwd=self.temp_path)
        self.assertEqual(result.exit_code, 0)
        self.assertEqual(result.stdout, "1.2.3")
        self.assertEqual(result.stderr, "")

    def test_main_reports_missing_manifest(self) -> None:
        missing = self.temp_path / "missing.toml"
        result = self._invoke_cli("name", "--manifest-path", str(missing))
        self.assertNotEqual(result.exit_code, 0)
        self.assertIn("does not exist", result.stderr)
        self.assertEqual(result.stdout, "")

    def test_main_reports_invalid_toml(self) -> None:
        manifest = self._write_manifest("not = [valid")
        result = self._invoke_cli("name", "--manifest-path", str(manifest))
        self.assertNotEqual(result.exit_code, 0)
        self.assertTrue(result.stderr)
        self.assertEqual(result.stdout, "")

    def test_main_reports_valid_toml_with_unexpected_structure(self) -> None:
        manifest = self._write_manifest(
            """
            [unexpected_section]
            foo = "bar"
            """
        )
        result = self._invoke_cli("name", "--manifest-path", str(manifest))
        self.assertNotEqual(result.exit_code, 0)
        self.assertIn("missing", result.stderr.lower())
        self.assertEqual(result.stdout, "")


if __name__ == "__main__":
    unittest.main()
