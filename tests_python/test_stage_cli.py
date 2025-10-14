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

# The shared ``workspace`` fixture is defined in ``tests_python.conftest``;
# keeping the dependency explicit here discourages recreating a local variant
# that would shadow the shared behaviour and reintroduce divergence.

REPO_ROOT = Path(__file__).resolve().parent.parent
SCRIPTS_DIR = REPO_ROOT / ".github" / "actions" / "stage" / "scripts"


class _StubCycloptsApp:
    """Minimal stand-in for :mod:`cyclopts` used during testing."""

    def __init__(self, *args: object, **kwargs: object) -> None:  # noqa: ARG002 - stub signature must match cyclopts.App
        self._handler: typ.Callable[..., object] | None = None

    def default(self, func: typ.Callable[..., object]) -> typ.Callable[..., object]:
        self._handler = func
        return func

    def __call__(self, *args: object, **kwargs: object) -> None:  # noqa: ARG002 - stub signature must match cyclopts.App
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


def _remove_sys_path_entry(entry: str) -> None:
    """Remove ``entry`` from ``sys.path`` if present, preferring index 0."""

    if sys.path and sys.path[0] == entry:
        del sys.path[0]
        return

    try:
        sys.path.remove(entry)
    except ValueError:
        pass


def _restore_module(name: str, previous: ModuleType | None) -> None:
    """Restore ``sys.modules[name]`` to ``previous`` or remove if it was absent."""

    if previous is not None:
        sys.modules[name] = previous
    else:
        sys.modules.pop(name, None)


@pytest.fixture
def stage_cli() -> ModuleType:
    """Import the CLI module with a stubbed :mod:`cyclopts`."""

    sys_path_entry = str(SCRIPTS_DIR)
    previous_stage = sys.modules.get("stage")
    previous_cyclopts = sys.modules.get("cyclopts")

    sys.path.insert(0, sys_path_entry)
    try:
        cyclopts_stub = ModuleType("cyclopts")
        cyclopts_stub.App = _StubCycloptsApp  # type: ignore[attr-defined]
        sys.modules["cyclopts"] = cyclopts_stub

        module = importlib.import_module("stage")
        yield module
    finally:
        _remove_sys_path_entry(sys_path_entry)
        _restore_module("stage", previous_stage)
        _restore_module("cyclopts", previous_cyclopts)


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
    assert artefact_map["binary_path"].endswith(
        "netsuke"
    ), "artefact map should expose the staged binary"
    assert outputs["binary_path"].endswith(
        "netsuke"
    ), "CLI output should provide the staged binary path"


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

    assert exc.value.code == 1, "CLI should exit with status 1 when outputs are missing"
