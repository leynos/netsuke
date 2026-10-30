#!/usr/bin/env python3
"""Write bounded release-operation summaries and telemetry.

The workflow calls this after staging, publication, or crate packaging to write
a GitHub summary and JSONL records. They use fixed operation, phase, outcome,
and error-category labels, and omit release tags, asset names, raw errors, and
tokens. Operation-specific phases keep unrelated packaging steps out.
"""

import dataclasses as dc
import json
import os
import time
import typing as typ
from pathlib import Path

if typ.TYPE_CHECKING:
    import collections.abc as cabc

VALID_OUTCOMES = frozenset({"success", "failure", "cancelled", "skipped", "unknown"})
PHASES = {
    "draft_release": ("Draft release", "draft_creation"),
    "artifact_download": ("Artefact download", "artifact_download"),
    "archive_hoist": ("Archive hoist", "archive_hoist"),
    "upload_plan_validation": ("Upload plan validation", "upload_plan_validation"),
    "crate_packaging": ("Crate packaging", "crate_packaging"),
    "artifact_upload": ("Artefact upload", "artifact_upload"),
}
#: Which phases each operation has. Stated per operation rather than shared,
#: because the phases are the operation's own boundaries: staging downloads and
#: hoists artefacts, publication adds draft creation, and packaging has neither
#: — it builds the crate and hands it to the next job. Reading a phase a job
#: does not have would report it "skipped" forever, which looks like a phase
#: that never fails rather than one that is not part of the operation.
OPERATION_PHASES = {
    "release_staging": (
        "draft_release",
        "artifact_download",
        "archive_hoist",
        "upload_plan_validation",
    ),
    "release_publication": (
        "draft_release",
        "artifact_download",
        "archive_hoist",
        "upload_plan_validation",
    ),
    "release_package": ("crate_packaging", "artifact_upload"),
}
OPERATIONS = frozenset(OPERATION_PHASES)
PHASE_ORDER = tuple(PHASES)
PHASE_OUTCOMES = frozenset({"success", "failure", "cancelled"})


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
    )


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
    )
    error_category = _error_category(safe_observation)
    records = _metric_records(safe_observation, error_category)
    records.extend(_trace_records(safe_observation))
    jsonl = "".join(
        json.dumps(record, sort_keys=True, separators=(",", ":")) + "\n"
        for record in records
    )
    upload_error_present = str(safe_observation.upload_error_present).lower()
    lines = [
        "## Release operation telemetry",
        "",
        f"- Operation: `{safe_observation.operation}`",
        f"- Dry run: `{str(safe_observation.dry_run).lower()}`",
        f"- Job status: `{safe_observation.job_status}`",
        f"- Error category: `{error_category}`",
        f"- Upload error output present: `{upload_error_present}`",
        "- Phase outcomes:",
    ]
    for phase in _phases(safe_observation.operation):
        step = safe_observation.steps[phase]
        duration = _duration_seconds(step.started_ns, step.ended_ns)
        duration_text = "unavailable" if duration is None else f"{duration:.3f}s"
        lines.append(f"  - `{phase}`: `{step.outcome}` ({duration_text})")
    return "\n".join(lines) + "\n", jsonl


def _optional_timestamp(value: str) -> int | None:
    """Parse an optional nanosecond timestamp emitted by a timer step."""
    return int(value) if value else None


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

    Raises
    ------
    ValueError
        For an invalid operation, flag, or timestamp.

    Examples
    --------
    >>> values = {"RELEASE_OPERATION": "release_staging",
    ...     "RELEASE_DRY_RUN": "true", "UPLOAD_ERROR_PRESENT": "false"}
    >>> observation_from_environment(values, ended_ns=1_600_000_000).ended_ns
    1600000000
    """
    operation = environment["RELEASE_OPERATION"]
    dry_run_value = environment["RELEASE_DRY_RUN"]
    if dry_run_value not in {"true", "false"}:
        message = "RELEASE_DRY_RUN must be true or false"
        raise ValueError(message)
    error_present_value = environment["UPLOAD_ERROR_PRESENT"]
    if error_present_value not in {"true", "false"}:
        message = "UPLOAD_ERROR_PRESENT must be true or false"
        raise ValueError(message)
    steps = {
        phase: StepState(
            outcome=environment.get(f"{phase.upper()}_OUTCOME", "skipped"),
            started_ns=_optional_timestamp(
                environment.get(f"{phase.upper()}_STARTED_NS", "")
            ),
            ended_ns=_optional_timestamp(
                environment.get(f"{phase.upper()}_ENDED_NS", "")
            ),
        )
        for phase in _phases(operation)
    }
    return OperationObservation(
        operation=operation,
        dry_run=dry_run_value == "true",
        job_status=environment.get("JOB_STATUS", "unknown"),
        started_ns=_optional_timestamp(environment.get("OPERATION_STARTED_NS", "")),
        ended_ns=ended_ns,
        steps=steps,
        upload_error_present=error_present_value == "true",
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
