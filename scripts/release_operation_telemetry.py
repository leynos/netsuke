#!/usr/bin/env python3
"""Render fixed-label release operation summaries, metrics, and trace events.

This module owns the bounded operation/phase records. The command adapter
reads GitHub's environment and writes the returned summary and JSONL; workflow
contracts mirror the operation catalogue and prohibit free-form labels.
"""

import dataclasses as dc
import json
import typing as typ

from release_operation_catalog import (
    OPERATION_PHASES,
    PHASE_OUTCOMES,
    PHASES,
    VALID_OUTCOMES,
    VALID_STATUS_CLASSES,
    publisher_context,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc


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


@dc.dataclass(frozen=True, slots=True)
class StepState:
    """Hold one workflow phase's bounded outcome and timing."""

    outcome: str
    started_ns: int | None
    ended_ns: int | None
    attempts: int | None = None
    status_class: str | None = None
    elapsed_ms: int | None = None


@dc.dataclass(frozen=True, slots=True)
class OperationObservation:
    """Collect the safe, bounded facts for one release operation."""

    operation: str
    dry_run: bool
    job_status: str
    started_ns: int | None
    ended_ns: int
    # ruff: ignore[quoted-annotation] - Python 3.12 evaluates annotations eagerly.
    steps: "cabc.Mapping[str, StepState]"
    upload_error_present: bool
    # ruff: ignore[quoted-annotation] - Python 3.12 evaluates annotations eagerly.
    context: "cabc.Mapping[str, str]" = dc.field(default_factory=dict)


def _outcome(value: str) -> str:
    """Return a known workflow outcome or the fixed unknown label."""
    return value if value in VALID_OUTCOMES else "unknown"


def _duration_seconds(started_ns: int | None, ended_ns: int | None) -> float | None:
    """Return a non-negative duration in seconds when both timestamps exist."""
    if started_ns is None or ended_ns is None:
        return None
    if ended_ns < started_ns:
        return None
    return round((ended_ns - started_ns) / 1_000_000_000, 3)


def _error_category(observation: OperationObservation) -> str:
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


def _metric_records(
    observation: OperationObservation, error_category: str
) -> list[dict[str, object]]:
    """Build low-cardinality operation and phase metrics."""
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
    for phase in _phases(observation.operation):
        step = observation.steps[phase]
        if step.outcome not in PHASE_OUTCOMES:
            continue
        records.append({
            "record_type": "metric",
            "metric": "release_phase_total",
            "value": 1,
            "phase": phase,
            "outcome": step.outcome,
            "error_category": _phase_error_category(phase, step.outcome),
            "operation": observation.operation,
            "dry_run": observation.dry_run,
        })
        duration = _duration_seconds(step.started_ns, step.ended_ns)
        if duration is not None:
            records.append({
                "record_type": "metric",
                "metric": "release_phase_duration_seconds",
                "value": duration,
                "phase": phase,
                "outcome": step.outcome,
                "error_category": _phase_error_category(phase, step.outcome),
                "operation": observation.operation,
                "dry_run": observation.dry_run,
            })
        if phase == "registry_download" and step.attempts is not None:
            common_download = {
                "operation": observation.operation,
                "dry_run": observation.dry_run,
            }
            records.append({
                "record_type": "metric",
                "metric": "release_registry_download_attempts",
                "value": step.attempts,
                **common_download,
            })
            if step.elapsed_ms is not None:
                records.append({
                    "record_type": "metric",
                    "metric": "release_registry_download_elapsed_ms",
                    "value": step.elapsed_ms,
                    **common_download,
                })
            if step.status_class is not None:
                records.append({
                    "record_type": "metric",
                    "metric": "release_registry_download_status_class",
                    "value": 1,
                    "status_class": step.status_class,
                    **common_download,
                })
    return records


def _phase_started_event(
    observation: OperationObservation, phase: str, step: StepState
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
    observation: OperationObservation, phase: str, step: StepState
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


def _trace_records(observation: OperationObservation) -> list[dict[str, object]]:
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


def _sanitised_step(observation: OperationObservation, phase: str) -> StepState:
    """Return one phase's bounded outcome and timing.

    Render an omitted phase as ``skipped`` rather than failing. A partial
    observability record is more useful during an incident than no record.

    Returns
    -------
    StepState
        Bounded outcome and timings, or an empty state when the phase is absent.
    """
    step = observation.steps.get(phase)
    if step is None:
        return StepState(outcome="skipped", started_ns=None, ended_ns=None)
    return StepState(
        outcome=_outcome(step.outcome),
        started_ns=step.started_ns,
        ended_ns=step.ended_ns,
        attempts=step.attempts,
        status_class=(
            step.status_class
            if step.status_class in VALID_STATUS_CLASSES
            else "unknown"
        ),
        elapsed_ms=step.elapsed_ms,
    )


def _append_retry_metrics(lines: list[str], step: StepState) -> None:
    """Append bounded retry facts when a phase records an attempt count.

    Examples
    --------
    >>> lines = []
    >>> _append_retry_metrics(
    ...     lines, StepState("success", 1, 2, attempts=2, status_class="2xx")
    ... )
    >>> lines[0]
    '    - Attempts: `2`'
    """
    if step.attempts is not None:
        status_class = step.status_class or "unknown"
        elapsed_text = (
            "unavailable" if step.elapsed_ms is None else f"{step.elapsed_ms}ms"
        )
        lines.extend((
            f"    - Attempts: `{step.attempts}`",
            f"    - Last HTTP status class: `{status_class}`",
            f"    - Retry duration: `{elapsed_text}`",
        ))


def _render_summary(observation: OperationObservation, error_category: str) -> str:
    """Render the bounded Markdown summary for one sanitised observation.

    Returns
    -------
    str
        The Markdown summary containing bounded outcomes and timings.

    Examples
    --------
    >>> phases = OPERATION_PHASES["publish_crates_io"]
    >>> steps = {phase: StepState("skipped", None, None) for phase in phases}
    >>> observation = OperationObservation(
    ...     "publish_crates_io", True, "success", None, 0, steps, False
    ... )
    >>> _render_summary(observation, "none").startswith(
    ...     "## Release operation telemetry"
    ... )
    True
    """
    upload_error_present = str(observation.upload_error_present).lower()
    lines = [
        "## Release operation telemetry",
        "",
        f"- Operation: `{observation.operation}`",
        f"- Dry run: `{str(observation.dry_run).lower()}`",
        f"- Job status: `{observation.job_status}`",
        f"- Error category: `{error_category}`",
        f"- Upload error output present: `{upload_error_present}`",
    ]
    if observation.context:
        lines.extend(("", "- Publisher context:"))
        for field in (
            "crate_name",
            "version",
            "registry_state",
            "action",
            "verification_verdict",
        ):
            value = observation.context.get(field)
            if value is not None:
                lines.append(f"  - `{field}`: `{value}`")
    lines.extend(("", "- Phase outcomes:"))
    for phase in _phases(observation.operation):
        step = observation.steps[phase]
        duration = _duration_seconds(step.started_ns, step.ended_ns)
        duration_text = "unavailable" if duration is None else f"{duration:.3f}s"
        lines.append(f"  - `{phase}`: `{step.outcome}` ({duration_text})")
        _append_retry_metrics(lines, step)
    return "\n".join(lines) + "\n"


def render_observation(observation: OperationObservation) -> tuple[str, str]:
    """Render the step summary and JSONL telemetry for an operation."""
    phases = _phases(observation.operation)
    safe_observation = OperationObservation(
        operation=observation.operation,
        dry_run=observation.dry_run,
        job_status=_outcome(observation.job_status),
        started_ns=observation.started_ns,
        ended_ns=observation.ended_ns,
        steps={phase: _sanitised_step(observation, phase) for phase in phases},
        upload_error_present=observation.upload_error_present,
        context=publisher_context(observation.context),
    )
    error_category = _error_category(safe_observation)
    records = _metric_records(safe_observation, error_category)
    records.extend(_trace_records(safe_observation))
    jsonl = "".join(
        json.dumps(record, sort_keys=True, separators=(",", ":")) + "\n"
        for record in records
    )
    return _render_summary(safe_observation, error_category), jsonl
