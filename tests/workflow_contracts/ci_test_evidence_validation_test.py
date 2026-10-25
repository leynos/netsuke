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
