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


@pytest.mark.parametrize(
    ("payload", "expected_error", "message"),
    [
        pytest.param(
            {"inputs": {"wix-extension-version": []}},
            resolver.ExtensionVersionShapeError,
            "wix-extension-version must be a JSON string",
            id="version-list",
        ),
        pytest.param(
            {"inputs": {"wix-extension-version": {}}},
            resolver.ExtensionVersionShapeError,
            "wix-extension-version must be a JSON string",
            id="version-object",
        ),
        pytest.param(
            {"inputs": {"wix-extension-version": 42}},
            resolver.ExtensionVersionShapeError,
            "wix-extension-version must be a JSON string",
            id="version-number",
        ),
        pytest.param(
            {"inputs": {"wix-extension-version": False}},
            resolver.ExtensionVersionShapeError,
            "wix-extension-version must be a JSON string",
            id="version-boolean",
        ),
        pytest.param(
            {"inputs": []},
            resolver.WorkflowInputsShapeError,
            "workflow inputs must be a JSON object",
            id="inputs-list",
        ),
        pytest.param(
            {"inputs": "not-a-mapping"},
            resolver.WorkflowInputsShapeError,
            "workflow inputs must be a JSON object",
            id="inputs-string",
        ),
        pytest.param(
            {"inputs": 42},
            resolver.WorkflowInputsShapeError,
            "workflow inputs must be a JSON object",
            id="inputs-number",
        ),
        pytest.param(
            [],
            resolver.WorkflowEventShapeError,
            "GitHub event payload must be a JSON object",
            id="event-list",
        ),
        pytest.param(
            "not-an-object",
            resolver.WorkflowEventShapeError,
            "GitHub event payload must be a JSON object",
            id="event-string",
        ),
        pytest.param(
            42,
            resolver.WorkflowEventShapeError,
            "GitHub event payload must be a JSON object",
            id="event-number",
        ),
    ],
)
def test_workflow_call_rejects_invalid_payload_shapes(
    tmp_path: Path,
    payload: object,
    expected_error: type[ValueError],
    message: str,
) -> None:
    """Reject malformed event and input values with a clear diagnostic."""
    event_path = tmp_path / "event.json"
    event_path.write_text(json.dumps(payload), encoding="utf-8")

    with pytest.raises(expected_error, match=message):
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
