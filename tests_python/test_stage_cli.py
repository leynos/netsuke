"""Behavioural tests for the staging CLI entry point."""

from __future__ import annotations

import importlib
import json
import sys
from pathlib import Path
from types import ModuleType

import pytest

from test_stage_common import _decode_output_file  # reuse helper

REPO_ROOT = Path(__file__).resolve().parent.parent
SCRIPTS_DIR = REPO_ROOT / ".github" / "actions" / "stage" / "scripts"


class _StubCycloptsApp:
    """Minimal stand-in for :mod:`cyclopts` used during testing."""

    def __init__(self, *args, **kwargs) -> None:  # noqa: D401 - simple stub
        self._handler = None

    def default(self, func):
        self._handler = func
        return func

    def __call__(self, *args, **kwargs):  # pragma: no cover - not exercised
        raise RuntimeError("Stub CLI should not be invoked directly")


@pytest.fixture
def workspace(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
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
    module = importlib.import_module("stage")
    yield module
    sys.path.remove(str(SCRIPTS_DIR))
    sys.modules.pop("stage", None)
    sys.modules.pop("cyclopts", None)


def _write_inputs(root: Path, target: str) -> None:
    bin_path = root / "target" / target / "release" / "netsuke"
    bin_path.parent.mkdir(parents=True, exist_ok=True)
    bin_path.write_bytes(b"binary")

    man_path = root / "target" / "generated-man" / target / "release" / "netsuke.1"
    man_path.parent.mkdir(parents=True, exist_ok=True)
    man_path.write_text(".TH NETSUKE 1", encoding="utf-8")

    licence = root / "LICENSE"
    licence.write_text("Copyright Netsuke", encoding="utf-8")


def test_stage_cli_stages_and_reports(
    stage_cli: ModuleType, workspace: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """The CLI should stage artefacts and emit GitHub outputs."""

    config_src = REPO_ROOT / ".github" / "release-staging.toml"
    config_copy = workspace / "release-staging.toml"
    config_copy.write_text(config_src.read_text(encoding="utf-8"), encoding="utf-8")

    target = "linux-x86_64"
    _write_inputs(workspace, "x86_64-unknown-linux-gnu")

    github_output = workspace / "outputs.txt"
    monkeypatch.setenv("GITHUB_OUTPUT", str(github_output))

    stage_cli.main(config_copy, target)

    outputs = _decode_output_file(github_output)
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
