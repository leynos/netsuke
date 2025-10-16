"""Derive release workflow modes for GitHub Actions.

The release workflow combines reusable invocations (``workflow_call``) with tag
pushes. This helper normalises the event payload into the booleans the rest of
our workflow needs and emits them via ``GITHUB_OUTPUT``.

Examples
--------
Running the helper during a dry run disables publishing and workflow artefact
uploads::

    $ GITHUB_EVENT_NAME=workflow_call \
      GITHUB_EVENT_PATH=event.json \
      GITHUB_OUTPUT=outputs \
      python determine_release_modes.py

    $ cat outputs
    dry_run=true
    should_publish=false
    should_upload_workflow_artifacts=false
"""

from __future__ import annotations

import json
import os
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Mapping


@dataclass(frozen=True)
class ReleaseModes:
    """Aggregate release settings derived from the workflow event."""

    dry_run: bool
    should_publish: bool
    should_upload_workflow_artifacts: bool

    def to_output_mapping(self) -> dict[str, str]:
        """Serialise the release modes into ``GITHUB_OUTPUT`` assignments."""

        return {
            "dry_run": _format_bool(self.dry_run),
            "should_publish": _format_bool(self.should_publish),
            "should_upload_workflow_artifacts": _format_bool(
                self.should_upload_workflow_artifacts
            ),
        }


def determine_release_modes(event_name: str, event: Mapping[str, Any]) -> ReleaseModes:
    """Derive release modes from a GitHub Actions event payload.

    Parameters
    ----------
    event_name:
        The ``github.event_name`` value describing how the workflow was
        triggered.
    event:
        The loaded JSON payload from ``GITHUB_EVENT_PATH``.

    Returns
    -------
    ReleaseModes
        A frozen dataclass describing whether the workflow is a dry run, should
        publish to a release, and may upload workflow artefacts.

    Raises
    ------
    ValueError
        If the ``event_name`` is unsupported or the event inputs contain values
        that cannot be coerced to booleans.

    Examples
    --------
    A tag push always publishes and uploads artefacts::

        >>> determine_release_modes("push", {})
        ReleaseModes(dry_run=False, should_publish=True,
        ... should_upload_workflow_artifacts=True)

    A dry-run workflow call disables publishing and artefact uploads::

        >>> determine_release_modes(
        ...     "workflow_call", {"inputs": {"dry-run": "true", "publish": "true"}}
        ... )
        ReleaseModes(dry_run=True, should_publish=False,
        ... should_upload_workflow_artifacts=False)
    """

    if event_name not in {"push", "workflow_call"}:
        msg = f"Unsupported event '{event_name}' for release workflow"
        raise ValueError(msg)

    inputs = _extract_inputs(event) if event_name == "workflow_call" else {}

    dry_run = _coerce_bool(inputs.get("dry-run"), default=False)
    if event_name == "push":
        should_publish = True
    else:
        should_publish = _coerce_bool(inputs.get("publish"), default=False)

    if dry_run:
        should_publish = False

    return ReleaseModes(
        dry_run=dry_run,
        should_publish=should_publish,
        should_upload_workflow_artifacts=not dry_run,
    )


def main() -> None:
    """Entry point for GitHub Actions steps."""

    event_name = os.environ["GITHUB_EVENT_NAME"]
    event_path = Path(os.environ["GITHUB_EVENT_PATH"]).resolve()
    event_payload = _load_event(event_path)

    modes = determine_release_modes(event_name, event_payload)
    _write_outputs(Path(os.environ["GITHUB_OUTPUT"]), modes)


def _load_event(event_path: Path) -> Mapping[str, Any]:
    """Load the JSON payload for the triggering event."""

    if not event_path.exists():
        return {}
    with event_path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def _extract_inputs(event: Mapping[str, Any]) -> Mapping[str, Any]:
    """Extract workflow inputs, tolerating empty payloads."""

    inputs = event.get("inputs", {})
    if isinstance(inputs, Mapping):
        return inputs
    msg = "workflow inputs must be a mapping"
    raise ValueError(msg)


def _coerce_bool(value: object, *, default: bool) -> bool:
    """Interpret GitHub input values as booleans.

    GitHub Actions forwards ``workflow_call`` inputs as strings, so we accept a
    variety of spellings. ``None`` or empty strings fall back to ``default``.
    """

    if value is None:
        return default
    if isinstance(value, bool):
        return value
    if isinstance(value, str):
        normalised = value.strip().lower()
        if not normalised:
            return default
        if normalised in {"1", "true", "yes", "on"}:
            return True
        if normalised in {"0", "false", "no", "off"}:
            return False
    msg = f"Cannot interpret {value!r} as boolean"
    raise ValueError(msg)


def _format_bool(value: bool) -> str:
    """Convert ``bool`` values into the lowercase strings Actions expects."""

    return "true" if value else "false"


def _write_outputs(output_path: Path, modes: ReleaseModes) -> None:
    """Append release mode outputs for the surrounding workflow."""

    mapping = modes.to_output_mapping()
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with output_path.open("a", encoding="utf-8") as handle:
        for key, value in mapping.items():
            handle.write(f"{key}={value}\n")


if __name__ == "__main__":
    main()
