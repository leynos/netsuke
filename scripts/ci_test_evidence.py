#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# dependencies = ["cyclopts==4.25.3"]
# ///
r"""Collect offline CI evidence from GitHub attempt/job JSON and logs.

Offline collector; runner-seconds are a cost proxy.

Usage (log: ``<job_id>.log``; inventory: ``<job_id>.json``):
    uv run scripts/ci_test_evidence.py --run run.json --jobs jobs.json \\
        --log-dir logs --telemetry-dir telemetry --variant control > out.jsonl
"""

import dataclasses as dc
import datetime as dt
import enum
import json
import re
import typing as typ
from pathlib import Path, PurePosixPath

from ci_test_evidence_logs import parse_log
from ci_test_evidence_telemetry import (
    apply_inventory,
    load_inventory,
    validate_inventory,
)
from ci_test_evidence_types import (
    OUTCOMES,
    PHASES,
    RUNNER_LABELS,
    UBICLOUD_SHAPE,
    normalize_step_number,
    validate_identities,
)

from cyclopts import App, Parameter

if typ.TYPE_CHECKING:
    from ci_test_evidence_types import AttemptCost, JobRecord

app = App(version_flags=[])


@dc.dataclass(frozen=True, slots=True, kw_only=True)
class EvidenceFiles:
    """Group saved evidence paths at the offline CLI boundary."""

    run: Path
    jobs: Path
    log_dir: Path | None = None
    telemetry_dir: Path | None = None


class ExperimentVariant(enum.StrEnum):
    """Name fixed experiment labels; Cyclopts matches enum names case-insensitively."""

    CONTROL = "control"
    TELEMETRY = "telemetry"
    CACHE_FINGERPRINTS_SHADOW = "cache_fingerprints_shadow"
    PUBLIC_API_PREP = "public_api_prep"
    PROFILE_POOL_4 = "profile_pool_4"


class CollectorInputs(typ.TypedDict, total=False):
    """Describe optional per-job evidence maps and a fixed experiment label."""

    logs: dict[str, str]
    inventories: dict[str, object]
    variant: str


def duration_seconds(start: str | None, end: str | None) -> float | None:
    """Return elapsed seconds, or null for missing or backwards timestamps.

    Returns
    -------
    float | None
        For example, ``duration_seconds(None, None)`` returns ``None``.
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


def _cohort(run: dict, job: dict) -> str:
    """Classify evidence; ``_cohort({}, {})`` returns ``"other"``."""
    workflow = PurePosixPath(str(run.get("path", "")).split("@", 1)[0]).name
    ubicloud = "ubicloud-standard-4-ubuntu-2404" in job.get("labels", [])
    trunk = run.get("event") == "push" and run.get("head_branch") == "main"
    if workflow == "coverage-main.yml" and job.get("name") == "coverage-upload":
        return "trunk_coverage" if ubicloud and trunk else "other"
    if workflow != "ci.yml" or job.get("name") != "build-test":
        return "other"
    if run.get("event") == "pull_request":
        if ubicloud:
            return "pr_coverage"
        return "fork_hosted" if "ubuntu-latest" in job.get("labels", []) else "other"
    return "trunk_cache_writer" if trunk and ubicloud else "other"


def _timestamp(value: object) -> str | None:
    """Keep aware times; ``_timestamp("bad")`` returns ``None``."""
    if not isinstance(value, str):
        return None
    try:
        parsed = dt.datetime.fromisoformat(value)
    except ValueError:
        return None
    return value if parsed.utcoffset() is not None else None


def _elapsed(
    entry: dict, *, start_field: str = "started_at", end_field: str = "completed_at"
) -> float | None:
    """Measure an API entry; ``_elapsed({})`` returns ``None``."""
    return duration_seconds(
        _timestamp(entry.get(start_field)), _timestamp(entry.get(end_field))
    )


def _outcome(value: object) -> str:
    """Normalize outcomes; ``_outcome("bad")`` returns ``"unknown"``."""
    return value if isinstance(value, str) and value in OUTCOMES else "unknown"


def _event(value: object) -> str:
    """Normalize events; ``_event("bad")`` returns ``"other"``."""
    known = {"push", "pull_request", "schedule", "workflow_dispatch"}
    return value if isinstance(value, str) and value in known else "other"


def _safe_sha(value: object) -> str | None:
    """Validate SHAs; ``_safe_sha("abc")`` returns ``None``."""
    return (
        value
        if isinstance(value, str) and re.fullmatch(r"[a-f0-9]{40}", value)
        else None
    )


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


def _attempt_cost(jobs: list[dict]) -> AttemptCost:
    """Sum jobs; one unknown duration makes the complete total ``None``."""
    durations = [_elapsed(job) for job in jobs]
    paid = [
        elapsed
        for job, elapsed in zip(jobs, durations, strict=True)
        if any(
            isinstance(label, str) and label.startswith("ubicloud-")
            for label in job.get("labels", [])
        )
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
        labels = [
            label
            for label in job.get("labels", [])
            if isinstance(label, str) and label.startswith("ubicloud-")
        ]
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


def _validate_attempt(run: dict, response: dict) -> None:
    """Validate attempts; jobs from attempt 1 cannot join attempt 2."""
    jobs = response["jobs"]
    total_count = response.get("total_count")
    if type(total_count) is not int or len(jobs) != total_count:
        message = "incomplete jobs response"
        raise ValueError(message)
    identities = [run.get("id"), run.get("run_attempt")]
    for job in jobs:
        identities.extend(job.get(field) for field in ("id", "run_id", "run_attempt"))
    validate_identities(identities)
    if len({job["id"] for job in jobs}) != len(jobs):
        message = "duplicate job identity"
        raise ValueError(message)
    for job in jobs:
        if (
            job.get("run_id") != run["id"]
            or job.get("run_attempt") != run["run_attempt"]
        ):
            message = "run or attempt mismatch"
            raise ValueError(message)


def collect_attempt(
    run: dict, response: dict, inputs: CollectorInputs | None = None
) -> list[JobRecord]:
    """Collect each job attempt, refusing mixed attempts or incomplete pages.

    Parameters
    ----------
    run
        Saved attempt-specific GitHub run API response.
    response
        Complete attempt-specific jobs response, including total_count.
    inputs
        Optional job-ID maps for logs and inventory, plus a fixed experiment
        variant label. Maps use string job IDs. In-memory inventories are
        checked against this run, attempt, and job before measurements are used.

    Returns
    -------
    list[JobRecord]
        Empty complete jobs give ``[]``; other jobs retain their outcomes.

    Raises
    ------
    ValueError
        If jobs are incomplete, mixed across attempts, or variant is unknown.
    TypeError
        If an in-memory evidence map or its values have the wrong shape.
    """
    values: CollectorInputs = {} if inputs is None else {**inputs}
    try:
        variant = ExperimentVariant(values.get("variant", ExperimentVariant.CONTROL))
    except TypeError, ValueError:
        message = "unknown experiment variant"
        raise ValueError(message) from None
    values["variant"] = variant.value
    jobs = response["jobs"]
    _validate_attempt(run, response)
    for key in ("logs", "inventories"):
        evidence_map = values.get(key, {})
        if not isinstance(evidence_map, dict) or not all(
            isinstance(job_id, str) for job_id in evidence_map
        ):
            message = f"{key} must be a string job-ID mapping"
            raise TypeError(message)
    if not all(isinstance(log, str) for log in values.get("logs", {}).values()):
        message = "logs must map job IDs to text"
        raise TypeError(message)
    cost = _attempt_cost(jobs)
    records = [_job_record(run, job, values) for job in jobs]
    for record in records:
        record.update(cost)
    return records


def _job_record(run: dict, job: dict, inputs: CollectorInputs) -> JobRecord:
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
        "cohort": _cohort(run, job),
        "runner_labels": [
            label
            for label in job.get("labels", [])
            if isinstance(label, str)
            and (label in RUNNER_LABELS or UBICLOUD_SHAPE.fullmatch(label))
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
    """Write JSONL from saved API responses and optional evidence files.

    For example, ``app(["--run", "run.json", "--jobs", "jobs.json"])``
    writes one JSON object for each saved job.

    Parameters
    ----------
    files
        Saved attempt, jobs, logs and optional inventory artefact paths.
    variant
        Fixed experiment label; defaults to control with no CI activation.
    """
    run = json.loads(files.run.read_text(encoding="utf-8"))
    jobs = json.loads(files.jobs.read_text(encoding="utf-8"))
    _validate_attempt(run, jobs)
    logs: dict[str, str] = {}
    if files.log_dir:
        for job in jobs["jobs"]:
            path = files.log_dir / f"{job['id']}.log"
            if path.is_file():
                logs[str(job["id"])] = path.read_text(encoding="utf-8")
    inventories: dict[str, object] = {}
    if files.telemetry_dir:
        for job in jobs["jobs"]:
            path = files.telemetry_dir / f"{job['id']}.json"
            if path.is_file():
                inventories[str(job["id"])] = load_inventory(
                    path, (run["id"], run["run_attempt"], job["id"])
                )
    for record in collect_attempt(
        run,
        jobs,
        {"logs": logs, "inventories": inventories, "variant": variant},
    ):
        print(json.dumps(record, sort_keys=True, separators=(",", ":")))


if __name__ == "__main__":
    app()
