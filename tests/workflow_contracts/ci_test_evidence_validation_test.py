"""Reject malformed saved jobs pages at collection and CLI boundaries."""

import json
import typing as typ

import pytest
from ci_test_evidence import app, collect_attempt

if typ.TYPE_CHECKING:
    from pathlib import Path


@pytest.mark.parametrize(
    "response",
    [
        {"total_count": 1},
        {"total_count": 1, "jobs": None},
        {"total_count": 1, "jobs": "not a list"},
        {"total_count": 1, "jobs": [None]},
    ],
    ids=["missing-jobs", "null-jobs", "non-list-jobs", "non-object-job"],
)
def test_collect_attempt_rejects_malformed_jobs_shapes(
    attempt: tuple[dict, dict], response: dict
) -> None:
    """Reject invalid jobs shapes before counting or extracting job fields."""
    run, _ = attempt

    with pytest.raises(TypeError, match="jobs response must contain a jobs list"):
        collect_attempt(run, response)


def test_cli_rejects_a_saved_jobs_error_body(
    attempt: tuple[dict, dict], tmp_path: Path
) -> None:
    """Report a saved API error body as invalid jobs data."""
    run, _ = attempt
    run_path, jobs_path = tmp_path / "run.json", tmp_path / "jobs.json"
    run_path.write_text(json.dumps(run), encoding="utf-8")
    jobs_path.write_text(
        json.dumps({"message": "Not Found", "status": "404"}),
        encoding="utf-8",
    )
    arguments = ["--run", str(run_path), "--jobs", str(jobs_path)]

    with pytest.raises(TypeError, match="jobs response must contain a jobs list"):
        app(arguments, result_action="return_value")


@pytest.mark.parametrize("key", ["0456", "789"])
@pytest.mark.parametrize("field", ["logs", "inventories"])
def test_collect_attempt_rejects_evidence_for_unknown_job(
    attempt: tuple[dict, dict], key: str, field: str
) -> None:
    """Bind each saved evidence entry to an exact job ID in the response."""
    run, jobs = attempt
    with pytest.raises(ValueError, match=f"{field} contains an unknown job ID"):
        collect_attempt(run, jobs, {field: {key: ""}})


@pytest.mark.parametrize(
    ("field", "value", "message"),
    [
        ("steps", None, "job steps must be a list of objects"),
        ("steps", [None], "job steps must be a list of objects"),
        ("labels", None, "job labels must be a list"),
        ("labels", "ubicloud-standard-4-ubuntu-2404", "job labels must be a list"),
    ],
)
def test_collect_attempt_rejects_malformed_job_containers(
    attempt: tuple[dict, dict], field: str, value: object, message: str
) -> None:
    """Reject malformed API container shapes before collection reads them."""
    run, jobs = attempt
    jobs["jobs"][0][field] = value
    with pytest.raises(TypeError, match=message):
        collect_attempt(run, jobs)
