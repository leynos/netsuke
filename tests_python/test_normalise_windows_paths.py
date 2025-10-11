"""Tests for the Windows path normalisation workflow helper."""

from __future__ import annotations

import importlib.util
import os
import subprocess
import sys
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parent.parent
SCRIPT_PATH = REPO_ROOT / ".github" / "workflows" / "scripts" / "normalise_windows_paths.py"


@pytest.fixture(scope="session")
def normalise_module() -> object:
    """Load the normalisation script as a module to access :func:`main`."""

    spec = importlib.util.spec_from_file_location(
        "normalise_windows_paths", SCRIPT_PATH
    )
    if spec is None or spec.loader is None:
        raise RuntimeError("Unable to import normalise_windows_paths module")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_main_writes_normalised_paths(
    normalise_module: object, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """The main entry point should persist normalised values to ``GITHUB_OUTPUT``."""

    github_output = tmp_path / "github" / "output.txt"
    github_output.parent.mkdir(parents=True, exist_ok=True)
    monkeypatch.setenv("BINARY_PATH", r"dist\\netsuke.exe")
    monkeypatch.setenv("LICENSE_PATH", r"dist\\LICENSE")
    monkeypatch.setenv("GITHUB_OUTPUT", str(github_output))

    exit_code = normalise_module.main()

    assert exit_code == 0, "Expected successful exit"
    content = github_output.read_text(encoding="utf-8")
    assert "binary_path=dist\\netsuke.exe" in content
    assert "license_path=dist\\LICENSE" in content


def test_script_invocation_writes_outputs(tmp_path: Path) -> None:
    """Executing the script via Python should emit the expected outputs."""

    github_output = tmp_path / "out.txt"
    env = os.environ | {
        "BINARY_PATH": r"C:\\stage\\netsuke.exe",
        "LICENSE_PATH": r"C:\\stage\\LICENSE",
        "GITHUB_OUTPUT": str(github_output),
    }

    completed = subprocess.run(
        [sys.executable, str(SCRIPT_PATH)],
        check=False,
        env=env,
        capture_output=True,
        text=True,
    )

    assert completed.returncode == 0, completed.stderr
    content = github_output.read_text(encoding="utf-8")
    assert "binary_path=C:\\stage\\netsuke.exe" in content
    assert "license_path=C:\\stage\\LICENSE" in content
