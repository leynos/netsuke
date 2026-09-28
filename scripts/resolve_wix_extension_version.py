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


class WorkflowInputsShapeError(ValueError):
    """Report workflow-call inputs that are not JSON objects."""

    def __init__(self) -> None:
        """Initialise the error for non-object workflow inputs."""
        super().__init__("workflow inputs must be a JSON object")


DEFAULT_EXTENSION_VERSION = "7"
app = App(config=cyclopts.config.Env("INPUT_", command=False))


def resolve_extension_version(event_name: str, event_path: str) -> str:
    """Resolve the requested extension version, falling back to version 7.

    Returns
    -------
    str
        The configured extension version or the default.

    Raises
    ------
    WorkflowInputsShapeError
        If workflow-call inputs are not a JSON object.

    Examples
    --------
    >>> resolve_extension_version("push", "unused")
    '7'
    """
    if event_name != "workflow_call":
        return DEFAULT_EXTENSION_VERSION

    payload = json.loads(pathlib.Path(event_path).read_text(encoding="utf-8"))
    inputs = payload.get("inputs")
    if inputs is None:
        inputs = {}
    elif not isinstance(inputs, dict):
        raise WorkflowInputsShapeError
    configured_version = inputs.get("wix-extension-version")
    if configured_version is None:
        version = ""
    elif isinstance(configured_version, str):
        version = configured_version
    else:
        version = json.dumps(configured_version)

    return version if version and version != "null" else DEFAULT_EXTENSION_VERSION


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
