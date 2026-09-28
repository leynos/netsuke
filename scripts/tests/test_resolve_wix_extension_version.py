"""Test WiX version resolution and GitHub output export."""

import json
import typing as typ

import pytest
import resolve_wix_extension_version as resolver

if typ.TYPE_CHECKING:
    from pathlib import Path


def test_push_uses_the_default_extension_version(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    """A push uses the same fallback as an empty reusable-workflow input."""
    event_path = tmp_path / "event.json"
    event_path.write_text("{}", encoding="utf-8")
    output_path = tmp_path / "github-output"
    output_path.write_text("existing=value\n", encoding="utf-8")
    monkeypatch.setenv("INPUT_EVENT_NAME", "push")
    monkeypatch.setenv("INPUT_EVENT_PATH", str(event_path))
    monkeypatch.setenv("GITHUB_OUTPUT", str(output_path))

    resolver.app([], result_action="return_value")

    assert output_path.read_text(encoding="utf-8") == "existing=value\nvalue=7\n", (
        "the default must append without replacing prior workflow output"
    )


def test_workflow_call_exports_the_requested_extension_version(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    """A configured reusable-workflow input reaches the output file."""
    event_path = tmp_path / "event.json"
    event_path.write_text(
        json.dumps({"inputs": {"wix-extension-version": "8.1.2"}}),
        encoding="utf-8",
    )
    output_path = tmp_path / "github-output"
    monkeypatch.setenv("INPUT_EVENT_NAME", "workflow_call")
    monkeypatch.setenv("INPUT_EVENT_PATH", str(event_path))
    monkeypatch.setenv("GITHUB_OUTPUT", str(output_path))

    resolver.app([], result_action="return_value")

    assert output_path.read_text(encoding="utf-8") == "value=8.1.2\n", (
        "the configured extension version must be exported"
    )


def test_workflow_call_with_null_inputs_uses_the_default(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    """Explicitly null workflow inputs fall back to the default version."""
    event_path = tmp_path / "event.json"
    event_path.write_text(json.dumps({"inputs": None}), encoding="utf-8")
    output_path = tmp_path / "github-output"
    monkeypatch.setenv("INPUT_EVENT_NAME", "workflow_call")
    monkeypatch.setenv("INPUT_EVENT_PATH", str(event_path))
    monkeypatch.setenv("GITHUB_OUTPUT", str(output_path))

    resolver.app([], result_action="return_value")

    assert output_path.read_text(encoding="utf-8") == "value=7\n", (
        "null workflow inputs must fall back to the default extension version"
    )


def test_workflow_call_rejects_an_invalid_event_file(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    """Malformed runner event JSON fails rather than silently using a default."""
    event_path = tmp_path / "event.json"
    event_path.write_text("{", encoding="utf-8")
    monkeypatch.setenv("INPUT_EVENT_NAME", "workflow_call")
    monkeypatch.setenv("INPUT_EVENT_PATH", str(event_path))
    monkeypatch.setenv("GITHUB_OUTPUT", str(tmp_path / "github-output"))

    with pytest.raises(json.JSONDecodeError):
        resolver.app([], result_action="return_value")
