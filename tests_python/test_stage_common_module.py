"""Sanity checks for the staging helper package."""

from __future__ import annotations

from pathlib import Path

import pytest


def test_public_interface(stage_common: object) -> None:
    """The package should expose the documented public API."""

    expected = {
        "ArtefactConfig",
        "RESERVED_OUTPUT_KEYS",
        "StageError",
        "StageResult",
        "StagingConfig",
        "load_config",
        "require_env_path",
        "stage_artefacts",
    }
    assert set(stage_common.__all__) == expected, (
        f"Public API mismatch: expected {expected}, got {set(stage_common.__all__)}"
    )


def test_stage_error_is_runtime_error(stage_common: object) -> None:
    """``StageError`` should subclass :class:`RuntimeError`."""

    error = stage_common.StageError("boom")
    assert isinstance(error, RuntimeError), (
        "StageError must subclass RuntimeError"
    )
    assert str(error) == "boom", (
        f"StageError message incorrect: expected 'boom', got '{error}'"
    )


def test_require_env_path_returns_path(stage_common: object, workspace: Path) -> None:
    """The environment helper should return a ``Path`` when set."""

    path = stage_common.require_env_path("GITHUB_WORKSPACE")
    assert path == workspace, (
        f"require_env_path returned incorrect path: expected {workspace}, got {path}"
    )


def test_require_env_path_missing_env(
    stage_common: object, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A missing environment variable should raise ``StageError``."""

    monkeypatch.delenv("GITHUB_WORKSPACE", raising=False)
    with pytest.raises(stage_common.StageError) as exc:
        stage_common.require_env_path("GITHUB_WORKSPACE")
    assert "Environment variable 'GITHUB_WORKSPACE' is not set" in str(exc.value)
