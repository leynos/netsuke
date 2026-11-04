#!/usr/bin/env python3
"""Write release operation summaries and JSONL from GitHub step outcomes.

This module is the CLI boundary for release observability. It reads GitHub
workflow facts and publisher step outputs, builds the ``OperationObservation``
and ``StepState`` values those surfaces describe, and asks
``release_operation_telemetry`` to render the bounded summary and JSONL
records. Keeping the parsing and file I/O here leaves the telemetry module a
pure formatter that the release jobs can unit test without GitHub inputs.
"""

import json
import os
import time
import typing as typ
from pathlib import Path

from release_operation_telemetry import (
    OperationObservation,
    StepState,
    _phases,
    render_observation,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc


def _optional_timestamp(value: str) -> int | None:
    """Parse an optional nanosecond timestamp emitted by a timer step."""
    return int(value) if value else None


def _optional_nonnegative_integer(value: str) -> int | None:
    """Parse an optional non-negative count or elapsed millisecond value."""
    if not value:
        return None
    parsed = int(value)
    if parsed < 0:
        message = "observability counts and durations must be non-negative"
        raise ValueError(message)
    return parsed


def _required_boolean(
    # ruff: ignore[quoted-annotation] - Python 3.12 evaluates annotations eagerly.
    environment: "cabc.Mapping[str, str]",
    name: str,
) -> bool:
    """Parse one required lowercase workflow boolean.

    Parameters
    ----------
    environment
        Values supplied by the workflow environment.
    name
        Exact environment key to parse.

    Returns
    -------
    bool
        Whether the value is ``true``.

    Raises
    ------
    ValueError
        If the key is neither ``true`` nor ``false``.

    Examples
    --------
    >>> _required_boolean({"RELEASE_DRY_RUN": "true"}, "RELEASE_DRY_RUN")
    True
    """
    value = environment[name]
    if value not in {"true", "false"}:
        message = f"{name} must be true or false"
        raise ValueError(message)
    return value == "true"


def _publisher_outputs(
    # ruff: ignore[quoted-annotation] - Python 3.12 evaluates annotations eagerly.
    environment: "cabc.Mapping[str, str]",
    operation: str,
) -> dict[str, str]:
    """Parse declared publisher outputs and reject non-string JSON values.

    Parameters
    ----------
    environment
        Values supplied by the workflow environment.
    operation
        Release operation selecting whether publisher output exists.

    Returns
    -------
    dict[str, str]
        Validated action output fields, or an empty mapping for other jobs.

    Examples
    --------
    >>> _publisher_outputs(
    ...     {"PUBLISHER_OUTPUTS": '{"action":"publish"}'}, "publish_crates_io"
    ... )
    {'action': 'publish'}
    """
    if operation != "publish_crates_io":
        return {}
    return _parse_publisher_outputs(environment.get("PUBLISHER_OUTPUTS", "{}"))


def _parse_publisher_outputs(value: str) -> dict[str, str]:
    """Decode publisher JSON, then validate its field mapping.

    Parameters
    ----------
    value
        JSON text emitted by the publisher composite action.

    Returns
    -------
    dict[str, str]
        Validated publisher output fields.

    Raises
    ------
    ValueError
        If the text is not valid JSON or does not contain string fields.

    Examples
    --------
    >>> _parse_publisher_outputs('{"action":"skip"}')
    {'action': 'skip'}
    """
    try:
        raw_outputs = json.loads(value)
    except json.JSONDecodeError as error:
        message = "PUBLISHER_OUTPUTS must be a JSON object"
        raise ValueError(message) from error
    try:
        return _string_output_mapping(raw_outputs)
    except TypeError as error:
        message = "PUBLISHER_OUTPUTS must contain string values"
        raise ValueError(message) from error


def _string_output_mapping(value: object) -> dict[str, str]:
    """Validate an object as a mapping of string publisher fields.

    Parameters
    ----------
    value
        Decoded JSON value to validate.

    Returns
    -------
    dict[str, str]
        The string fields supplied by the publisher action.

    Raises
    ------
    TypeError
        If the JSON value is not an object with string keys and values.

    Examples
    --------
    >>> _string_output_mapping({"action": "publish"})
    {'action': 'publish'}
    """
    if not isinstance(value, dict):
        message = "PUBLISHER_OUTPUTS must contain string values"
        raise TypeError(message)
    outputs: dict[str, str] = {}
    for name, field_value in value.items():
        if not isinstance(name, str) or not isinstance(field_value, str):
            message = "PUBLISHER_OUTPUTS must contain string values"
            raise TypeError(message)
        outputs[name] = field_value
    return outputs


def _phase_environment(
    operation: str,
    publisher_outputs: dict[str, str],
    # ruff: ignore[quoted-annotation] - Python 3.12 evaluates annotations eagerly.
    environment: "cabc.Mapping[str, str]",
) -> dict[str, str]:
    """Normalize publisher phase keys to the workflow environment spelling.

    Parameters
    ----------
    operation
        Release operation selecting the phase key format.
    publisher_outputs
        Validated outputs from the publisher action.
    environment
        Workflow values used by operations without action outputs.

    Returns
    -------
    dict[str, str]
        Phase fields keyed by uppercase underscore names.

    Examples
    --------
    >>> _phase_environment(
    ...     "publish_crates_io", {"registry-probe-outcome": "success"}, {}
    ... )
    {'REGISTRY_PROBE_OUTCOME': 'success'}
    """
    if operation == "publish_crates_io":
        return {
            name.replace("-", "_").upper(): value
            for name, value in publisher_outputs.items()
        }
    return dict(environment)


def _phase_state(
    phase: str,
    # ruff: ignore[quoted-annotation] - Python 3.12 evaluates annotations eagerly.
    values: "cabc.Mapping[str, str]",
) -> StepState:
    """Build one typed phase state from normalized environment keys.

    Parameters
    ----------
    phase
        Catalogue phase name.
    values
        Workflow or normalized publisher phase values.

    Returns
    -------
    StepState
        Parsed outcome, timestamps, status class, and retry observations.

    Examples
    --------
    >>> state = _phase_state(
    ...     "registry_probe", {"REGISTRY_PROBE_OUTCOME": "success"}
    ... )
    >>> state.outcome
    'success'
    """
    prefix = phase.upper()
    status_class = values.get(f"{prefix}_STATUS_CLASS", "") or None
    return StepState(
        outcome=values.get(f"{prefix}_OUTCOME", "skipped"),
        started_ns=_optional_timestamp(values.get(f"{prefix}_STARTED_NS", "")),
        ended_ns=_optional_timestamp(values.get(f"{prefix}_ENDED_NS", "")),
        attempts=_optional_nonnegative_integer(values.get(f"{prefix}_ATTEMPTS", "")),
        status_class=status_class,
        elapsed_ms=_optional_nonnegative_integer(
            values.get(f"{prefix}_ELAPSED_MS", "")
        ),
    )


def observation_from_environment(
    # ruff: ignore[quoted-annotation] - Python 3.12 evaluates annotations eagerly.
    environment: "cabc.Mapping[str, str]",
    *,
    ended_ns: int,
) -> OperationObservation:
    """Build an observation using the caller's time for deterministic parsing.

    Parameters
    ----------
    environment
        Workflow environment values used to build the observation.
    ended_ns
        Nanosecond end timestamp captured by the caller.

    Returns
    -------
    OperationObservation
        Parsed values with the supplied end timestamp.

    Examples
    --------
    >>> values = {"RELEASE_OPERATION": "release_staging",
    ...     "RELEASE_DRY_RUN": "true", "UPLOAD_ERROR_PRESENT": "false"}
    >>> observation_from_environment(values, ended_ns=1_600_000_000).ended_ns
    1600000000
    """
    operation = environment["RELEASE_OPERATION"]
    dry_run = _required_boolean(environment, "RELEASE_DRY_RUN")
    upload_error_present = _required_boolean(environment, "UPLOAD_ERROR_PRESENT")
    publisher_outputs = _publisher_outputs(environment, operation)
    phase_values = _phase_environment(operation, publisher_outputs, environment)
    steps = {phase: _phase_state(phase, phase_values) for phase in _phases(operation)}
    return OperationObservation(
        operation=operation,
        dry_run=dry_run,
        job_status=environment.get("JOB_STATUS", "unknown"),
        started_ns=_optional_timestamp(environment.get("OPERATION_STARTED_NS", "")),
        ended_ns=ended_ns,
        steps=steps,
        upload_error_present=upload_error_present,
        context=publisher_outputs,
    )


def main(
    # ruff: ignore[quoted-annotation] - Python 3.12 evaluates annotations eagerly.
    environment: "cabc.Mapping[str, str] | None" = None,
) -> None:
    """Write a summary and local JSONL; publication retains the JSONL artifact."""
    values = os.environ if environment is None else environment
    observation = observation_from_environment(values, ended_ns=time.time_ns())
    summary, jsonl = render_observation(observation)
    summary_path = Path(values["GITHUB_STEP_SUMMARY"])
    telemetry_path = Path(values["OBSERVABILITY_PATH"])
    with summary_path.open("a", encoding="utf-8") as stream:
        stream.write(summary)
    telemetry_path.write_text(jsonl, encoding="utf-8")


if __name__ == "__main__":
    main()
