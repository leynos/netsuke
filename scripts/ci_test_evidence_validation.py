"""Validate saved job pages and optional evidence maps before collection.

The collector rejects incomplete or cross-attempt records so every output row
remains bound to the run attempt that produced it.
"""

import typing as typ

from ci_test_evidence_types import CollectorInputs, validate_identities


def validate_attempt_response(run: dict, response: dict) -> None:
    """Validate that a jobs page belongs completely to one run attempt.

    Parameters
    ----------
    run
        Saved GitHub run API response for one attempt.
    response
        Saved jobs API response with ``jobs`` and ``total_count`` fields.

    Malformed pages fail validation before the collector records measurements.
    The delegated checks choose the exception that matches the invalid
    response or conflicting identity.
    """
    jobs = response["jobs"]
    _validate_complete_jobs_response(jobs, response.get("total_count"))
    _validate_job_identities(run, jobs)
    _validate_job_attempts(run, jobs)


def _validate_complete_jobs_response(jobs: list[dict], total_count: object) -> None:
    """Reject a paginated API response that omitted jobs."""
    if type(total_count) is not int or len(jobs) != total_count:
        message = "incomplete jobs response"
        raise ValueError(message)


def _validate_job_identities(run: dict, jobs: list[dict]) -> None:
    """Require unique, positive run and job identifiers."""
    identities = [run.get("id"), run.get("run_attempt")]
    for job in jobs:
        identities.extend(job.get(field) for field in ("id", "run_id", "run_attempt"))
    validate_identities(identities)
    if len({job["id"] for job in jobs}) != len(jobs):
        message = "duplicate job identity"
        raise ValueError(message)


def _validate_job_attempts(run: dict, jobs: list[dict]) -> None:
    """Reject jobs whose run or attempt differs from the supplied run."""
    for job in jobs:
        if (
            job.get("run_id") != run["id"]
            or job.get("run_attempt") != run["run_attempt"]
        ):
            message = "run or attempt mismatch"
            raise ValueError(message)


def validate_evidence_maps(values: CollectorInputs) -> None:
    """Validate optional evidence maps before recording measurements.

    Parameters
    ----------
    values
        Collector inputs that may contain string-keyed logs and inventories.

    Raises
    ------
    TypeError
        If either value is not a mapping with string job IDs, or a log value
        is not text.
    """
    for key in ("logs", "inventories"):
        evidence_map = values.get(key, {})
        match evidence_map:
            case dict() as mapping if all(_is_string(job_id) for job_id in mapping):
                continue
            case _:
                message = f"{key} must be a string job-ID mapping"
                raise TypeError(message)
    _validate_log_values(values.get("logs", {}))


def _validate_log_values(logs: dict[str, str]) -> None:
    """Require saved log values to be strings before parsing them."""
    if not all(_is_string(log) for log in logs.values()):
        message = "logs must map job IDs to text"
        raise TypeError(message)


def _is_string(value: object) -> typ.TypeGuard[str]:
    """Narrow arbitrary input to a string without coercion."""
    match value:
        case str():
            return True
        case _:
            return False
