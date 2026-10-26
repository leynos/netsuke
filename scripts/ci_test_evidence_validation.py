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

    Raises
    ------
    TypeError
        If the response does not contain a list of job objects.
    """
    match response:
        case {"jobs": list() as jobs} if all(_is_mapping(job) for job in jobs):
            pass
        case _:
            message = "jobs response must contain a jobs list"
            raise TypeError(message)
    _validate_complete_jobs_response(jobs, response.get("total_count"))
    _validate_job_identities(run, jobs)
    _validate_job_attempts(run, jobs)
    _validate_job_fields(jobs)


def _is_mapping(value: object) -> typ.TypeGuard[dict]:
    """Identify JSON objects without coercion."""
    match value:
        case dict():
            return True
        case _:
            return False


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


def _validate_job_fields(jobs: list[dict]) -> None:
    """Validate the collection fields on every job.

    Examples
    --------
    A job without ``steps`` or ``labels`` uses accepted empty lists.
    """
    for job in jobs:
        _validate_job_steps(job.get("steps", []))
        _validate_job_labels(job.get("labels", []))


def _validate_job_steps(steps: object) -> None:
    """Require steps to be a list of JSON objects.

    Raises
    ------
    TypeError
        If steps is not a list of mappings.

    Examples
    --------
    An omitted steps field defaults to an accepted empty list; ``[None]`` is
    rejected.
    """
    if not isinstance(steps, list) or not all(_is_mapping(step) for step in steps):
        message = "job steps must be a list of objects"
        raise TypeError(message)


def _validate_job_labels(labels: object) -> None:
    """Require labels to use the JSON list container.

    Raises
    ------
    TypeError
        If labels is not a list.

    Examples
    --------
    An omitted labels field defaults to an accepted empty list.
    """
    if not isinstance(labels, list):
        message = "job labels must be a list"
        raise TypeError(message)


def validate_evidence_maps(values: CollectorInputs, job_ids: set[str]) -> None:
    """Validate optional evidence maps before recording measurements.

    Parameters
    ----------
    values
        Collector inputs that may contain string-keyed logs and inventories.
    job_ids
        Exact string forms of the job IDs in the complete jobs response.

    Raises
    ------
    TypeError
        If either value is not a mapping with string job IDs, or a log value
        is not text.
    ValueError
        If an evidence map refers to a job outside the complete response.

    Examples
    --------
    An evidence key ``"456"`` matches job ID 456; ``"0456"`` does not.
    """
    for key in ("logs", "inventories"):
        evidence_map = values.get(key, {})
        match evidence_map:
            case dict() as mapping if all(_is_string(job_id) for job_id in mapping):
                pass
            case _:
                message = f"{key} must be a string job-ID mapping"
                raise TypeError(message)
        if not mapping.keys() <= job_ids:
            message = f"{key} contains an unknown job ID"
            raise ValueError(message)
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
