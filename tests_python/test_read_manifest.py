"""Tests for the read_manifest helper script."""

from __future__ import annotations

import importlib.util
import os
import subprocess
import sys
from pathlib import Path
from tempfile import TemporaryDirectory
from textwrap import dedent
from typing import Any

import unittest


REPO_ROOT = Path(__file__).resolve().parent.parent
SCRIPT_PATH = REPO_ROOT / ".github" / "workflows" / "scripts" / "read_manifest.py"


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

    def test_main_reads_manifest_path_argument(self) -> None:
        manifest = self._write_manifest(
            """
            [package]
            name = "netsuke"
            version = "1.2.3"
            """
        )
        result = subprocess.run(
            [sys.executable, str(SCRIPT_PATH), "name", "--manifest-path", str(manifest)],
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.returncode, 0)
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
        env = os.environ.copy()
        env["CARGO_TOML_PATH"] = str(manifest)
        result = subprocess.run(
            [sys.executable, str(SCRIPT_PATH), "version"],
            check=False,
            capture_output=True,
            text=True,
            env=env,
            cwd=self.temp_path,
        )
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stdout, "1.2.3")
        self.assertEqual(result.stderr, "")

    def test_main_reports_missing_manifest(self) -> None:
        missing = self.temp_path / "missing.toml"
        result = subprocess.run(
            [sys.executable, str(SCRIPT_PATH), "name", "--manifest-path", str(missing)],
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("does not exist", result.stderr)
        self.assertEqual(result.stdout, "")

    def test_main_reports_invalid_toml(self) -> None:
        manifest = self._write_manifest("not = [valid")
        result = subprocess.run(
            [sys.executable, str(SCRIPT_PATH), "name", "--manifest-path", str(manifest)],
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue(result.stderr)
        self.assertEqual(result.stdout, "")


if __name__ == "__main__":
    unittest.main()
