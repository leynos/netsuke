#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# dependencies = ["cyclopts>=4.25.3,<5"]
# ///
"""Resolve and export the WiX extension version for a release workflow."""

import json
import pathlib
import typing as typ

import cyclopts
from cyclopts import App, Parameter


class WorkflowEventShapeError(ValueError):
    """Report a GitHub event payload that is not a JSON object."""

    def __init__(self) -> None:
        """Initialize the error for a non-object event payload."""
        super().__init__("GitHub event payload must be a JSON object")


class WorkflowInputsShapeError(ValueError):
    """Report workflow-call inputs that are not JSON objects."""

    def __init__(self) -> None:
        """Initialize the error for non-object workflow inputs."""
        super().__init__("workflow inputs must be a JSON object")


class ExtensionVersionShapeError(ValueError):
    """Report a WiX extension version that is not a JSON string."""

    def __init__(self) -> None:
        """Initialize the error for a non-string extension version."""
        super().__init__("wix-extension-version must be a JSON string")


DEFAULT_EXTENSION_VERSION = "7"
app = App(config=cyclopts.config.Env("INPUT_", command=False))


def load_workflow_call_inputs(event_path: str) -> dict[str, object]:
    """Return workflow-call inputs from the event file.

    Keep this parser in the release script because it owns this event shape.

    Returns
    -------
    dict[str, object]
        The workflow inputs, or an empty mapping when they are absent.

    Raises
    ------
    WorkflowEventShapeError
        If the event payload is not a JSON object.
    WorkflowInputsShapeError
        If workflow-call inputs are not a JSON object.
    """
    payload = json.loads(pathlib.Path(event_path).read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise WorkflowEventShapeError

    inputs = payload.get("inputs")
    if inputs is None:
        return {}
    if not isinstance(inputs, dict):
        raise WorkflowInputsShapeError
    return inputs


def resolve_extension_version(event_name: str, event_path: str) -> str:
    """Resolve the requested extension version, falling back to version 7.

    Returns
    -------
    str
        The configured extension version or the default.

    Raises
    ------
    ExtensionVersionShapeError
        If the configured extension version is not a JSON string.

    Examples
    --------
    >>> resolve_extension_version("push", "unused")
    '7'
    """
    if event_name != "workflow_call":
        return DEFAULT_EXTENSION_VERSION

    inputs = load_workflow_call_inputs(event_path)
    configured_version = inputs.get("wix-extension-version")
    if configured_version is None:
        version = ""
    elif isinstance(configured_version, str):
        version = configured_version
    else:
        raise ExtensionVersionShapeError

    return DEFAULT_EXTENSION_VERSION if version in {"", "null"} else version


@app.default
def export_extension_version(
    *,
    event_name: typ.Annotated[str, Parameter(required=True)],
    event_path: typ.Annotated[str, Parameter(required=True)],
    output_path: typ.Annotated[str, Parameter(env_var="GITHUB_OUTPUT", required=True)],
) -> None:
    """Write the resolved WiX extension version to the workflow output file.

    Examples
    --------
    With ``INPUT_EVENT_NAME=push`` and ``GITHUB_OUTPUT=/tmp/output``, append
    ``value=7`` to that output file.
    """
    version = resolve_extension_version(event_name, event_path)
    with pathlib.Path(output_path).open("a", encoding="utf-8") as output:
        output.write(f"value={version}\n")


if __name__ == "__main__":
    app()
