"""Shared fixtures for the staging helper test suite."""

from __future__ import annotations

import importlib
import sys
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parent.parent
MODULE_DIR = REPO_ROOT / ".github" / "actions" / "stage" / "scripts"


@pytest.fixture(scope="session")
def stage_common() -> object:
    """Load the staging helper package once for reuse across tests."""
    sys_path = str(MODULE_DIR)

    sys.path.insert(0, sys_path)
    try:
        return importlib.import_module("stage_common")
    finally:
        sys.path.remove(sys_path)


@pytest.fixture
def staging_package(stage_common: object) -> object:
    """Expose the staging package for API boundary assertions."""

    return importlib.import_module("stage_common.staging")


@pytest.fixture
def staging_pipeline(stage_common: object) -> object:
    """Expose the staging pipeline module for unit-level assertions."""

    return importlib.import_module("stage_common.staging.pipeline")


@pytest.fixture
def staging_output(stage_common: object) -> object:
    """Expose the staging output helpers for direct testing."""

    return importlib.import_module("stage_common.staging.output")


@pytest.fixture
def staging_resolution(stage_common: object) -> object:
    """Expose the path resolution helpers for direct testing."""

    return importlib.import_module("stage_common.staging.resolution")


@pytest.fixture
def workspace(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    """Create an isolated workspace and set ``GITHUB_WORKSPACE`` accordingly."""
    root = tmp_path / "workspace"
    root.mkdir()
    monkeypatch.setenv("GITHUB_WORKSPACE", str(root))
    return root
