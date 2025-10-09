"""Lightweight checks for the staging composite GitHub Action."""

from __future__ import annotations

from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
ACTION_FILE = REPO_ROOT / ".github" / "actions" / "stage" / "action.yml"


def test_action_declares_required_outputs() -> None:
    """The composite action should expose the expected top-level outputs."""

    content = ACTION_FILE.read_text(encoding="utf-8")

    assert "steps.run-stage.outputs.binary_path" in content
    assert "steps.run-stage.outputs.man_path" in content
    assert "steps.run-stage.outputs.license_path" in content


def test_action_installs_uv() -> None:
    """The composite action must ensure ``uv`` is available."""

    content = ACTION_FILE.read_text(encoding="utf-8")

    assert "uses: astral-sh/setup-uv@" in content
    assert "python-version: '3.11'" in content


def test_action_invokes_cli_script() -> None:
    """The action should run the staging script via ``uv run``."""

    content = ACTION_FILE.read_text(encoding="utf-8")

    assert "uv run" in content
    assert "scripts/stage.py" in content
