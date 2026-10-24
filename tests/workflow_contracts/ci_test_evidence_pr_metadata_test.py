"""Specify CLI handling of separately saved pull request metadata."""

import json
import typing as typ

from ci_test_evidence import app

if typ.TYPE_CHECKING:
    from pathlib import Path

    import pytest


def test_cli_uses_matching_saved_pull_request_metadata(
    attempt: tuple[dict, dict], tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Join the offline PR response before classifying a native workflow run."""
    run, jobs = attempt
    run.update(
        event="pull_request",
        path=".github/workflows/ci.yml",
        repository={"id": 42},
        head_repository={"id": 42},
        pull_requests=[
            {
                "id": 9001,
                "number": 17,
                "base": {"repo": {"id": 42}},
                "head": {"repo": {"id": 42}},
                "url": "https://api.github.com/repos/leynos/netsuke/pulls/17",
            }
        ],
    )
    jobs["jobs"][0].update(name="build-test", labels=["ubuntu-latest"])
    run_path = tmp_path / "run.json"
    jobs_path = tmp_path / "jobs.json"
    pull_request_path = tmp_path / "pull-request.json"
    run_path.write_text(json.dumps(run), encoding="utf-8")
    jobs_path.write_text(json.dumps(jobs), encoding="utf-8")
    pull_request_path.write_text(
        json.dumps({
            "id": 9001,
            "number": 17,
            "base": {"repo": {"id": 42}},
            "user": {"login": "dependabot[bot]"},
        }),
        encoding="utf-8",
    )

    app(
        [
            "--run",
            str(run_path),
            "--jobs",
            str(jobs_path),
            "--pull-request",
            str(pull_request_path),
        ],
        result_action="return_value",
    )
    record = json.loads(capsys.readouterr().out)

    assert record["cohort"] == "dependabot_hosted", (
        "The CLI must match saved PR author provenance to the native run"
    )
