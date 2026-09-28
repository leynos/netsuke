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


@pytest.mark.parametrize(
    "payload",
    [
        {},
        {"inputs": None},
        {"inputs": {}},
        {"inputs": {"wix-extension-version": "null"}},
    ],
    ids=["missing-inputs", "null-inputs", "missing-version", "null-sentinel"],
)
def test_workflow_call_without_a_version_uses_the_default(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path, payload: dict[str, object]
) -> None:
    """Missing inputs, keys, or the null sentinel use the default version."""
    event_path = tmp_path / "event.json"
    event_path.write_text(json.dumps(payload), encoding="utf-8")
    output_path = tmp_path / "github-output"
    monkeypatch.setenv("INPUT_EVENT_NAME", "workflow_call")
    monkeypatch.setenv("INPUT_EVENT_PATH", str(event_path))
    monkeypatch.setenv("GITHUB_OUTPUT", str(output_path))

    resolver.app([], result_action="return_value")

    assert output_path.read_text(encoding="utf-8") == "value=7\n", (
        "missing inputs or the null sentinel must use the default version"
    )


@pytest.mark.parametrize("version", [[], {}, 42, False])
def test_workflow_call_rejects_non_string_versions(
    tmp_path: Path, version: object
) -> None:
    """Reject malformed version values instead of serializing them to output."""
    event_path = tmp_path / "event.json"
    event_path.write_text(
        json.dumps({"inputs": {"wix-extension-version": version}}),
        encoding="utf-8",
    )

    with pytest.raises(
        resolver.ExtensionVersionShapeError,
        match="wix-extension-version must be a JSON string",
    ):
        resolver.resolve_extension_version("workflow_call", str(event_path))


@pytest.mark.parametrize("inputs", [[], "not-a-mapping", 42])
def test_workflow_call_rejects_non_object_inputs(
    tmp_path: Path, inputs: object
) -> None:
    """Non-object workflow inputs fail with a clear diagnostic."""
    event_path = tmp_path / "event.json"
    event_path.write_text(json.dumps({"inputs": inputs}), encoding="utf-8")

    with pytest.raises(
        resolver.WorkflowInputsShapeError,
        match="workflow inputs must be a JSON object",
    ):
        resolver.resolve_extension_version("workflow_call", str(event_path))


@pytest.mark.parametrize("payload", [[], "not-an-object", 42])
def test_workflow_call_rejects_non_object_event_payload(
    tmp_path: Path, payload: object
) -> None:
    """Non-object event payloads fail with a clear diagnostic."""
    event_path = tmp_path / "event.json"
    event_path.write_text(json.dumps(payload), encoding="utf-8")

    with pytest.raises(
        resolver.WorkflowEventShapeError,
        match="GitHub event payload must be a JSON object",
    ):
        resolver.resolve_extension_version("workflow_call", str(event_path))


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
