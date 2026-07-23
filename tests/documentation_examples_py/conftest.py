"""Fixtures for isolated documentation-example tests."""

from __future__ import annotations

import os
from pathlib import Path

import pytest
from documentation_examples import NetsukeRunner

pytest_plugins = ("cmd_mox.pytest_plugin",)

REPOSITORY = Path(__file__).resolve().parents[2]


@pytest.fixture(name="repository", scope="session")
def repository_fixture() -> Path:
    """Return the repository root."""
    return REPOSITORY


@pytest.fixture(name="netsuke", scope="session")
def netsuke_fixture(repository: Path) -> NetsukeRunner:
    """Return a Cuprum-backed runner for the built Netsuke binary."""
    binary = Path(os.environ.get("NETSUKE_BIN", repository / "target/debug/netsuke"))
    if not binary.is_file():
        pytest.fail(f"Netsuke binary does not exist: {binary}")
    return NetsukeRunner(binary=binary)


@pytest.fixture(autouse=True)
def isolate_netsuke_environment(monkeypatch: pytest.MonkeyPatch) -> None:
    """Remove host configuration selectors before every example test."""
    for name in (
        "NETSUKE_CONFIG",
        "NETSUKE_CONFIG_PATH",
        "NETSUKE_OUTPUT_FORMAT",
    ):
        monkeypatch.delenv(name, raising=False)
