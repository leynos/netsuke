"""Build bounded metric and trace records from release observations.

This module owns pure in-memory record construction for the telemetry renderer.
The renderer sanitises workflow values and composes these private builders.
"""

import typing as typ

from release_operation_catalog import (
    OPERATION_PHASES,
    PHASE_OUTCOMES,
    PHASES,
)

if typ.TYPE_CHECKING:
    from release_operation_telemetry import OperationObservation, StepState


def _phases(operation: str) -> tuple[str, ...]:
    """Return the ordered phases for ``operation``.

    Returns
    -------
    tuple[str, ...]
        The declared phase names in run order.

    Raises
    ------
    ValueError
        If ``operation`` is unknown.
    """
    try:
        return OPERATION_PHASES[operation]
    except KeyError:
        message = f"unsupported release operation: {operation!r}"
        raise ValueError(message) from None


def _duration_seconds(started_ns: int | None, ended_ns: int | None) -> float | None:
    """Return a non-negative duration in seconds when both timestamps exist."""
    if started_ns is None or ended_ns is None:
        return None
    if ended_ns < started_ns:
        return None
    return round((ended_ns - started_ns) / 1_000_000_000, 3)


def _error_category(
    # ruff: ignore[quoted-annotation] - Python 3.12 evaluates annotations eagerly.
    observation: "OperationObservation",
) -> str:
    """Choose the first fixed failure category reported by the workflow."""
    if observation.upload_error_present:
        return "upload_plan_validation"
    for phase in _phases(observation.operation):
        if observation.steps[phase].outcome == "failure":
            return PHASES[phase][1]
    if observation.job_status == "failure":
        return "other"
    if observation.job_status == "cancelled":
        return "cancelled"
    return "none"


def _phase_error_category(phase: str, outcome: str) -> str:
    """Return the fixed error category for a phase result."""
    if outcome == "failure":
        return PHASES[phase][1]
    return "cancelled" if outcome == "cancelled" else "none"


def _operation_metric_records(
    # ruff: ignore[quoted-annotation] - Python 3.12 evaluates annotations eagerly.
    observation: "OperationObservation",
    error_category: str,
) -> list[dict[str, object]]:
    """Build the total and duration records for one release operation."""
    common = {
        "operation": observation.operation,
        "dry_run": observation.dry_run,
        "outcome": observation.job_status,
        "error_category": error_category,
    }
    records: list[dict[str, object]] = [
        {
            "record_type": "metric",
            "metric": "release_operation_total",
            "value": 1,
            **common,
        }
    ]
    operation_duration = _duration_seconds(observation.started_ns, observation.ended_ns)
    if operation_duration is not None:
        records.append({
            "record_type": "metric",
            "metric": "release_operation_duration_seconds",
            "value": operation_duration,
            **common,
        })
    return records


def _registry_download_metric_records(
    # ruff: ignore[quoted-annotation] - Python 3.12 evaluates annotations eagerly.
    observation: "OperationObservation",
    # ruff: ignore[quoted-annotation] - Python 3.12 evaluates annotations eagerly.
    step: "StepState",
) -> list[dict[str, object]]:
    """Build bounded retry metrics for a completed registry download."""
    if step.attempts is None:
        return []
    common = {
        "operation": observation.operation,
        "dry_run": observation.dry_run,
    }
    records: list[dict[str, object]] = [
        {
            "record_type": "metric",
            "metric": "release_registry_download_attempts",
            "value": step.attempts,
            **common,
        }
    ]
    if step.elapsed_ms is not None:
        records.append({
            "record_type": "metric",
            "metric": "release_registry_download_elapsed_ms",
            "value": step.elapsed_ms,
            **common,
        })
    if step.status_class is not None:
        records.append({
            "record_type": "metric",
            "metric": "release_registry_download_status_class",
            "value": 1,
            "status_class": step.status_class,
            **common,
        })
    return records


def _phase_metric_records(
    # ruff: ignore[quoted-annotation] - Python 3.12 evaluates annotations eagerly.
    observation: "OperationObservation",
    phase: str,
) -> list[dict[str, object]]:
    """Build the outcome and duration records for one workflow phase."""
    step = observation.steps[phase]
    if step.outcome not in PHASE_OUTCOMES:
        return []
    error_category = _phase_error_category(phase, step.outcome)
    common = {
        "phase": phase,
        "outcome": step.outcome,
        "error_category": error_category,
        "operation": observation.operation,
        "dry_run": observation.dry_run,
    }
    records: list[dict[str, object]] = [
        {
            "record_type": "metric",
            "metric": "release_phase_total",
            "value": 1,
            **common,
        }
    ]
    duration = _duration_seconds(step.started_ns, step.ended_ns)
    if duration is not None:
        records.append({
            "record_type": "metric",
            "metric": "release_phase_duration_seconds",
            "value": duration,
            **common,
        })
    if phase == "registry_download":
        records.extend(_registry_download_metric_records(observation, step))
    return records


def _metric_records(
    # ruff: ignore[quoted-annotation] - Python 3.12 evaluates annotations eagerly.
    observation: "OperationObservation",
    error_category: str,
) -> list[dict[str, object]]:
    """Compose low-cardinality operation and ordered phase metrics."""
    records = _operation_metric_records(observation, error_category)
    for phase in _phases(observation.operation):
        records.extend(_phase_metric_records(observation, phase))
    return records


def _phase_started_event(
    # ruff: ignore[quoted-annotation] - Python 3.12 evaluates annotations eagerly.
    observation: "OperationObservation",
    phase: str,
    # ruff: ignore[quoted-annotation] - Python 3.12 evaluates annotations eagerly.
    step: "StepState",
) -> dict[str, object] | None:
    """Build a phase-start event when the workflow supplied its timestamp."""
    if step.started_ns is None:
        return None
    return {
        "record_type": "trace_event",
        "event": "release_phase_started",
        "operation": observation.operation,
        "phase": phase,
        "timestamp_ns": step.started_ns,
        "dry_run": observation.dry_run,
        **observation.context,
    }


def _phase_terminal_event(
    # ruff: ignore[quoted-annotation] - Python 3.12 evaluates annotations eagerly.
    observation: "OperationObservation",
    phase: str,
    # ruff: ignore[quoted-annotation] - Python 3.12 evaluates annotations eagerly.
    step: "StepState",
) -> dict[str, object] | None:
    """Build a phase completion or interruption event when its outcome allows."""
    if step.ended_ns is not None:
        record: dict[str, object] = {
            "record_type": "trace_event",
            "event": "release_phase_completed",
            "operation": observation.operation,
            "phase": phase,
            "timestamp_ns": step.ended_ns,
            "outcome": step.outcome,
            "error_category": _phase_error_category(phase, step.outcome),
            "dry_run": observation.dry_run,
            **observation.context,
        }
        duration = _duration_seconds(step.started_ns, step.ended_ns)
        if duration is not None:
            record["duration_seconds"] = duration
        return record
    if step.outcome == "cancelled":
        return {
            "record_type": "trace_event",
            "event": "release_phase_interrupted",
            "operation": observation.operation,
            "phase": phase,
            "timestamp_ns": observation.ended_ns,
            "outcome": step.outcome,
            "error_category": "cancelled",
            "dry_run": observation.dry_run,
            **observation.context,
        }
    return None


def _trace_records(
    # ruff: ignore[quoted-annotation] - Python 3.12 evaluates annotations eagerly.
    observation: "OperationObservation",
) -> list[dict[str, object]]:
    """Build bounded start and completion events for executed phases."""
    records: list[dict[str, object]] = []
    for phase in _phases(observation.operation):
        step = observation.steps[phase]
        if step.outcome not in PHASE_OUTCOMES:
            continue
        records.extend(
            event
            for event in (
                _phase_started_event(observation, phase, step),
                _phase_terminal_event(observation, phase, step),
            )
            if event is not None
        )
    return records
