"""Behavioural tests for the staging CLI entry point."""

from __future__ import annotations

import importlib
import json
import sys
import typing as typ
from pathlib import Path
from types import ModuleType

import pytest
from stage_test_helpers import decode_output_file, write_workspace_inputs

REPO_ROOT = Path(__file__).resolve().parent.parent
SCRIPTS_DIR = REPO_ROOT / ".github" / "actions" / "stage" / "scripts"


class _StubCycloptsApp:
    """Minimal stand-in for :mod:`cyclopts` used during testing."""

    def __init__(self, *args: object, **kwargs: object) -> None:
        self._handler: typ.Callable[..., object] | None = None

    def default(self, func: typ.Callable[..., object]) -> typ.Callable[..., object]:
        self._handler = func
        return func

    def __call__(self, *args: object, **kwargs: object) -> None:
        """Prevent the stub from being invoked directly."""
        message = "Stub CLI should not be invoked directly"
        raise RuntimeError(message)  # pragma: no cover - not exercised


@pytest.fixture
def workspace(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    """Provide an isolated workspace and set ``GITHUB_WORKSPACE``."""
    root = tmp_path / "workspace"
    root.mkdir()
    monkeypatch.setenv("GITHUB_WORKSPACE", str(root))
    return root


@pytest.fixture
def stage_cli(monkeypatch: pytest.MonkeyPatch) -> ModuleType:
    """Import the CLI module with a stubbed :mod:`cyclopts`."""
    sys.path.insert(0, str(SCRIPTS_DIR))
    monkeypatch.setitem(sys.modules, "cyclopts", ModuleType("cyclopts"))
    cyclopts_module = sys.modules["cyclopts"]
    cyclopts_module.App = _StubCycloptsApp  # type: ignore[attr-defined]
    yield importlib.import_module("stage")
    sys.path.remove(str(SCRIPTS_DIR))
    sys.modules.pop("stage", None)
    sys.modules.pop("cyclopts", None)


def test_stage_cli_stages_and_reports(
    stage_cli: ModuleType, workspace: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """The CLI should stage artefacts and emit GitHub outputs."""
    config_src = REPO_ROOT / ".github" / "release-staging.toml"
    config_copy = workspace / "release-staging.toml"
    config_copy.write_text(config_src.read_text(encoding="utf-8"), encoding="utf-8")

    target = "linux-x86_64"
    write_workspace_inputs(workspace, "x86_64-unknown-linux-gnu")

    github_output = workspace / "outputs.txt"
    monkeypatch.setenv("GITHUB_OUTPUT", str(github_output))

    stage_cli.main(config_copy, target)

    outputs = decode_output_file(github_output)
    artefact_map = json.loads(outputs["artefact_map"])
    assert artefact_map["binary_path"].endswith("netsuke")
    assert outputs["binary_path"].endswith("netsuke")


def test_stage_cli_requires_github_output(
    stage_cli: ModuleType, workspace: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """The CLI should exit with an error when ``GITHUB_OUTPUT`` is missing."""
    config_src = REPO_ROOT / ".github" / "release-staging.toml"
    config_copy = workspace / "release-staging.toml"
    config_copy.write_text(config_src.read_text(encoding="utf-8"), encoding="utf-8")

    monkeypatch.delenv("GITHUB_OUTPUT", raising=False)

    with pytest.raises(SystemExit) as exc:
        stage_cli.main(config_copy, "linux-x86_64")

    assert exc.value.code == 1
