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
    VALID_OUTCOMES,
    VALID_STATUS_CLASSES,
    publisher_context,
)
from release_operation_records import (
    _duration_seconds,
    _error_category,
    _metric_records,
    _phases,
    _trace_records,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc


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
    >>> phases = _phases("publish_crates_io")
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
    context_lines = [
        f"  - `{field}`: `{value}`"
        for field in (
            "crate_name",
            "version",
            "registry_state",
            "action",
            "verification_verdict",
        )
        if (value := observation.context.get(field)) is not None
    ]
    if context_lines:
        lines.extend(("", "- Publisher context:", *context_lines))
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
