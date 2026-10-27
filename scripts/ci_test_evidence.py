#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# dependencies = ["cyclopts==4.25.3"]
# ///
r"""Collect offline CI evidence; runner-seconds are a cost proxy."""

import dataclasses as dc
import datetime as dt
import enum
import json
import re
import typing as typ
from pathlib import (
    Path,  # ruff: ignore[typing-only-standard-library-import] - Cyclopts resolves CLI annotations at registration.
)

from ci_test_evidence_cohorts import classify_cohort
from ci_test_evidence_files import read_inventories, read_logs
from ci_test_evidence_logs import parse_log
from ci_test_evidence_telemetry import (
    apply_inventory,
    validate_inventory,
)
from ci_test_evidence_types import (
    OUTCOMES,
    PHASES,
    UBICLOUD_SHAPE,
    AttemptCost,
    CollectorInputs,
    JobRecord,
    is_runner_label,
    normalize_step_number,
)
from ci_test_evidence_validation import (
    validate_attempt_response,
    validate_evidence_maps,
)

from cyclopts import App, Parameter

app = App(version_flags=[])


@dc.dataclass(frozen=True, slots=True, kw_only=True)
class EvidenceFiles:
    """Group saved evidence paths at the offline CLI boundary."""

    run: Path
    jobs: Path
    pull_request: Path | None = None
    log_dir: Path | None = None
    telemetry_dir: Path | None = None


class ExperimentVariant(enum.StrEnum):
    """Name fixed experiment labels; Cyclopts matches enum names case-insensitively."""

    CONTROL = "control"
    TELEMETRY = "telemetry"
    CACHE_FINGERPRINTS_SHADOW = "cache_fingerprints_shadow"
    PUBLIC_API_PREP = "public_api_prep"
    PROFILE_POOL_4 = "profile_pool_4"


def duration_seconds(start: str | None, end: str | None) -> float | None:
    """Return elapsed seconds, or null for missing or backwards timestamps.

    Parameters
    ----------
    start
        ISO 8601 timestamp with a UTC offset, or ``None``.
    end
        ISO 8601 timestamp with a UTC offset, or ``None``.

    Returns
    -------
    float | None
        For example, ``duration_seconds(None, None)`` returns ``None``.
        Timezone-naive, malformed, or backwards timestamps also return
        ``None``.
    """
    if start is None or end is None:
        return None
    try:
        first, last = (dt.datetime.fromisoformat(value) for value in (start, end))
        if first.utcoffset() is None or last.utcoffset() is None:
            return None
        elapsed = (last - first).total_seconds()
    except ValueError, TypeError:
        return None
    return elapsed if elapsed >= 0 else None


def _timestamp(value: object) -> str | None:
    """Keep aware times; ``_timestamp("bad")`` returns ``None``."""
    match value:
        case str():
            try:
                parsed = dt.datetime.fromisoformat(value)
            except ValueError:
                return None
            return value if parsed.utcoffset() is not None else None
        case _:
            return None


def _elapsed(
    entry: dict, *, start_field: str = "started_at", end_field: str = "completed_at"
) -> float | None:
    """Measure an API entry; ``_elapsed({})`` returns ``None``."""
    return duration_seconds(
        _timestamp(entry.get(start_field)), _timestamp(entry.get(end_field))
    )


def _outcome(value: object) -> str:
    """Normalize outcomes; ``_outcome("bad")`` returns ``"unknown"``."""
    match value:
        case str() if value in OUTCOMES:
            return value
        case _:
            return "unknown"


def _event(value: object) -> str:
    """Normalize events; ``_event("bad")`` returns ``"other"``."""
    known = {"push", "pull_request", "schedule", "workflow_dispatch"}
    match value:
        case str() if value in known:
            return value
        case _:
            return "other"


def _safe_sha(value: object) -> str | None:
    """Validate SHAs; ``_safe_sha("abc")`` returns ``None``."""
    match value:
        case str() if re.fullmatch(r"[a-f0-9]{40}", value):
            return value
        case _:
            return None


def _job_phases(job: dict) -> list[dict[str, object]]:
    """Read steps; ``_job_phases({})`` returns ``[]``."""
    return [
        {
            "number": normalize_step_number(step.get("number")),
            "phase": _step_phase(step.get("name")),
            "outcome": _outcome(step.get("conclusion")),
            "started_at": _timestamp(step.get("started_at")),
            "completed_at": _timestamp(step.get("completed_at")),
            "seconds": _elapsed(step),
        }
        for step in job.get("steps", [])
    ]


def _step_phase(name: object) -> str:
    """Map steps; ``_step_phase(None)`` returns ``"other"``."""
    match name:
        case str() if name.startswith("Install "):
            return "tool_installation"
        case str() if name.startswith("Post "):
            return "post_job"
        case str():
            return PHASES.get(name, "other")
        case _:
            return "other"


def _is_ubicloud_label(label: object) -> bool:
    """Recognize paid labels; ``_is_ubicloud_label("ubicloud-x")`` is true."""
    match label:
        case str() if label.startswith("ubicloud-"):
            return True
        case _:
            return False


def _ubicloud_labels(job: dict) -> list[str]:
    """Return paid runner labels; ``_ubicloud_labels({})`` is ``[]``."""
    return [label for label in job.get("labels", []) if _is_ubicloud_label(label)]


def _attempt_cost(jobs: list[dict]) -> AttemptCost:
    """Sum jobs; one unknown duration makes the complete total ``None``."""
    durations = [_elapsed(job) for job in jobs]
    paid = [
        elapsed
        for job, elapsed in zip(jobs, durations, strict=True)
        if _ubicloud_labels(job)
    ]
    return {
        "attempt_runner_seconds": _sum_complete(durations),
        "attempt_observed_runner_seconds": sum(
            value for value in durations if value is not None
        ),
        "attempt_unknown_duration_jobs": sum(value is None for value in durations),
        "attempt_ubicloud_runner_seconds": _sum_complete(paid),
        "attempt_ubicloud_seconds_by_shape": _paid_by_shape(jobs),
        "cost_basis": "runner_seconds_proxy",
        "billing_consumption": None,
        "storage_transfer_bytes": None,
    }


def _paid_by_shape(jobs: list[dict]) -> dict[str, float | None]:
    """Group paid shapes; ``_paid_by_shape([])`` returns ``{}``."""
    groups: dict[str, list[float | None]] = {}
    for job in jobs:
        labels = _ubicloud_labels(job)
        if not labels:
            continue
        label = next(
            (label for label in labels if UBICLOUD_SHAPE.fullmatch(label)),
            "unclassified",
        )
        groups.setdefault(label, []).append(_elapsed(job))
    return {label: _sum_complete(values) for label, values in groups.items()}


def _sum_complete(values: list[float | None]) -> float | None:
    """Sum known time; ``_sum_complete([1, None])`` returns ``None``."""
    if any(value is None for value in values):
        return None
    return sum(value for value in values if value is not None)


def collect_attempt(
    run: dict,
    response: dict,
    inputs: CollectorInputs | None = None,
    *,
    pull_request: dict | None = None,
) -> list[JobRecord]:
    """Collect each job attempt, refusing mixed attempts or incomplete pages.

    Parameters
    ----------
    run
        Saved attempt-specific GitHub run API response.
    response
        Complete attempt-specific jobs response, including total_count.
    inputs
        Optional job-ID maps for evidence and a fixed experiment variant.
    pull_request
        Optional saved PR API response, used only after matching run identity.

    Incomplete job pages, cross-attempt records, malformed evidence maps, and
    unknown variants are rejected before any record is emitted.

    Returns
    -------
    list[JobRecord]
        Empty complete jobs give ``[]``; other jobs retain their outcomes.

    """
    validate_attempt_response(run, response)
    values = _normalized_inputs(inputs)
    jobs = response["jobs"]
    validate_evidence_maps(values, {str(job["id"]) for job in jobs})
    cost = _attempt_cost(jobs)
    records = [_job_record(run, job, values, pull_request) for job in jobs]
    for record in records:
        record.update(cost)
    return records


def _normalized_inputs(inputs: CollectorInputs | None) -> CollectorInputs:
    """Copy inputs and normalize their experiment variant."""
    values: CollectorInputs = {} if inputs is None else {**inputs}
    try:
        variant = ExperimentVariant(values.get("variant", ExperimentVariant.CONTROL))
    except TypeError, ValueError:
        message = "unknown experiment variant"
        raise ValueError(message) from None
    values["variant"] = variant.value

    return values


def _job_record(
    run: dict, job: dict, inputs: CollectorInputs, pull_request: dict | None
) -> JobRecord:
    """Build one record; absent logs give ``log_available=False``."""
    evidence = parse_log(inputs.get("logs", {}).get(str(job["id"]), ""))
    inventory = inputs.get("inventories", {}).get(str(job["id"]))
    if inventory is not None:
        apply_inventory(
            evidence,
            validate_inventory(inventory, (run["id"], run["run_attempt"], job["id"])),
        )
    run_url = f"https://github.com/leynos/netsuke/actions/runs/{run['id']}"
    return {
        "schema": 2,
        "run_id": run["id"],
        "job_id": job["id"],
        "run_attempt": run["run_attempt"],
        "run_url": run_url,
        "job_url": f"{run_url}/job/{job['id']}",
        "event": _event(run.get("event")),
        "source_sha": _safe_sha(run.get("head_sha")),
        "tested_merge_sha": evidence["tested_merge_sha"],
        "variant": inputs.get("variant", "control"),
        "relevant_diff": None,
        "comparison": "observational",
        "cohort": classify_cohort(run, job, pull_request),
        "runner_labels": [
            label for label in job.get("labels", []) if is_runner_label(label)
        ],
        "outcome": _outcome(job.get("conclusion")),
        "attempt_outcome": _outcome(run.get("conclusion")),
        "started_at": _timestamp(job.get("started_at")),
        "completed_at": _timestamp(job.get("completed_at")),
        "whole_job_seconds": _elapsed(job),
        "attempt_start_to_job_start_seconds": duration_seconds(
            _timestamp(run.get("run_started_at")), _timestamp(job.get("started_at"))
        ),
        "queue_and_provisioning_seconds": None,
        "steps": _job_phases(job),
        "evidence": evidence,
        "log_available": str(job["id"]) in inputs.get("logs", {}),
        "telemetry_available": inventory is not None,
    }


@app.default
def main(
    *,
    files: typ.Annotated[EvidenceFiles, Parameter(name="*")],
    variant: ExperimentVariant = ExperimentVariant.CONTROL,
) -> None:
    """Write JSONL from saved API responses.

    For example, ``app(["--run", "run.json", "--jobs", "jobs.json"])``
    writes one record for each saved job.
    """
    run, jobs, pull_request = _read_saved_attempt(files)
    validate_attempt_response(run, jobs)
    inputs: CollectorInputs = {
        "logs": read_logs(files.log_dir, jobs["jobs"]),
        "inventories": read_inventories(files.telemetry_dir, run, jobs["jobs"]),
        "variant": variant,
    }
    _write_records(collect_attempt(run, jobs, inputs, pull_request=pull_request))


def _read_saved_attempt(
    files: EvidenceFiles,
) -> tuple[dict, dict, dict | None]:
    """Load the run, jobs and optional pull request response from local files."""
    run = json.loads(files.run.read_text(encoding="utf-8"))
    jobs = json.loads(files.jobs.read_text(encoding="utf-8"))
    pull_request_path = files.pull_request
    response = (
        json.loads(pull_request_path.read_text(encoding="utf-8"))
        if pull_request_path is not None
        else None
    )
    pull_request = response if isinstance(response, dict) else None
    return run, jobs, pull_request


def _write_records(records: list[JobRecord]) -> None:
    """Write one compact JSON object per collected job attempt."""
    for record in records:
        print(json.dumps(record, sort_keys=True, separators=(",", ":")))


if __name__ == "__main__":
    app()
