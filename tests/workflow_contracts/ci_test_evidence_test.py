"""Specify offline, attempt-scoped CI evidence without inventing missing facts."""

import datetime as dt
import json
import typing as typ

import pytest
from ci_test_evidence import CollectorInputs, app, collect_attempt, duration_seconds
from hypothesis import given
from hypothesis import strategies as st

if typ.TYPE_CHECKING:
    from pathlib import Path


@pytest.mark.parametrize(
    ("start", "end", "expected"),
    [
        (None, "2026-10-12T00:00:00Z", None),
        ("invalid", "2026-10-12T00:00:00Z", None),
        ("2026-10-12T00:00:02Z", "2026-10-12T00:00:01Z", None),
        ("2026-10-12T00:00:00Z", "2026-10-12T00:00:01.500Z", 1.5),
    ],
)
def test_missing_or_backwards_time_is_not_zero(
    start: str | None, end: str | None, expected: float | None
) -> None:
    """Unknown durations must never look like free execution."""
    assert duration_seconds(start, end) == expected, (
        "Missing or reversed timestamps must not imply free execution"
    )


@given(start=st.integers(0, 86400), elapsed=st.integers(-86400, 86400))
def test_observed_duration_respects_ordering(start: int, elapsed: int) -> None:
    """Preserve timestamp differences and reject reversed clocks across a day."""
    first = dt.datetime(2026, 10, 12, tzinfo=dt.UTC) + dt.timedelta(seconds=start)
    last = first + dt.timedelta(seconds=elapsed)
    assert duration_seconds(first.isoformat(), last.isoformat()) == (
        elapsed if elapsed >= 0 else None
    ), "Elapsed time must preserve ordering across date boundaries"


@pytest.fixture
def attempt() -> tuple[dict, dict]:
    """Supply a complete attempt boundary with one Linux job."""
    return {
        "id": 123,
        "run_attempt": 2,
        "event": "pull_request",
        "head_branch": "main",
        "path": ".github/workflows/ci.yml",
        "run_started_at": "2026-10-12T00:00:00Z",
        "head_sha": "a" * 40,
        "created_at": "2026-10-12T00:00:00Z",
        "html_url": "https://github.com/leynos/netsuke/actions/runs/123",
    }, {
        "total_count": 1,
        "jobs": [
            {
                "id": 456,
                "run_id": 123,
                "run_attempt": 2,
                "name": "build-test",
                "labels": ["ubicloud-standard-4-ubuntu-2404"],
                "conclusion": "success",
                "started_at": "2026-10-12T00:00:05Z",
                "completed_at": "2026-10-12T00:00:15Z",
                "steps": [],
            }
        ],
    }


@pytest.mark.parametrize(
    ("number", "expected"),
    [(0, 0), (3, 3), (-1, None), (True, None), ("3", None), (1.5, None), (None, None)],
)
def test_step_numbers_require_nonnegative_integers(
    attempt: tuple[dict, dict], number: object, expected: int | None
) -> None:
    """Keep malformed step numbers out of published evidence."""
    run, jobs = attempt
    jobs["jobs"][0]["steps"] = [{"number": number}]

    record = collect_attempt(run, jobs)[0]

    assert record["steps"][0]["number"] == expected, (
        "Only non-negative integer step numbers are valid evidence"
    )


@pytest.mark.parametrize(
    ("inputs", "message"),
    [
        ({"logs": None}, "logs must be a string job-ID mapping"),
        ({"logs": {456: "captured log"}}, "logs must be a string job-ID mapping"),
        ({"logs": {"456": None}}, "logs must map job IDs to text"),
        ({"inventories": {456: {}}}, "inventories must be a string job-ID mapping"),
    ],
)
def test_collect_attempt_rejects_malformed_evidence_maps(
    attempt: tuple[dict, dict], inputs: object, message: str
) -> None:
    """Reject malformed maps and values before evidence lookup begins."""
    run, jobs = attempt

    with pytest.raises(TypeError, match=message):
        collect_attempt(run, jobs, typ.cast("CollectorInputs", inputs))


def test_whole_job_includes_post_work_and_missing_evidence(
    attempt: tuple[dict, dict],
) -> None:
    """Use job API endpoints rather than summed test durations."""
    run, jobs = attempt
    record = collect_attempt(run, jobs)[0]
    assert record["whole_job_seconds"] == 10, (
        "Whole-job duration must use job endpoints"
    )
    assert record["cohort"] == "pr_coverage", (
        "Eligible PR coverage must have its own cohort"
    )
    assert record["tested_merge_sha"] is None, (
        "Missing checkout evidence must remain unknown"
    )
    assert record["evidence"]["resources"]["peak_rss_mib"] is None, (
        "System memory must not substitute for process RSS"
    )
    assert record["attempt_runner_seconds"] == 10, (
        "Attempt cost must include the complete job"
    )
    assert record["schema"] == 2, (
        "The attempt-start field uses the current record schema"
    )
    assert record["queue_and_provisioning_seconds"] is None, (
        "Queue and provisioning must not be inferred from creation time"
    )


def test_attempt_start_delay_uses_current_attempt_timestamp(
    attempt: tuple[dict, dict],
) -> None:
    """Exclude the gap between run attempts from start-latency evidence."""
    run, jobs = attempt
    run.update(
        created_at="2026-10-12T00:00:00Z",
        run_started_at="2026-10-12T00:10:00Z",
        run_attempt=2,
    )
    jobs["jobs"][0].update(
        started_at="2026-10-12T00:10:05Z",
        completed_at="2026-10-12T00:10:15Z",
    )
    record = collect_attempt(run, jobs)[0]
    assert record["attempt_start_to_job_start_seconds"] == 5, (
        "Rerun latency must start at the current attempt"
    )

    run.pop("run_started_at")
    missing_start = collect_attempt(run, jobs)[0]
    assert missing_start["attempt_start_to_job_start_seconds"] is None, (
        "Missing attempt start must not fall back to original run creation"
    )


@pytest.mark.parametrize(
    "case",
    [
        ("pull_request", "ci.yml", "build-test", "ubuntu-latest", "fork_hosted"),
        ("pull_request", "ci.yml", "build-test", "unknown", "other"),
        (
            "pull_request",
            "kani-ci.yml",
            "build-test",
            "ubicloud-standard-4-ubuntu-2404",
            "other",
        ),
        (
            "push",
            "ci.yml",
            "build-test",
            "ubicloud-standard-4-ubuntu-2404",
            "trunk_cache_writer",
        ),
        (
            "push",
            "coverage-main.yml",
            "coverage-upload",
            "ubicloud-standard-4-ubuntu-2404",
            "trunk_coverage",
        ),
        (
            "push",
            "other-coverage-main.yml",
            "coverage-upload",
            "ubicloud-standard-4-ubuntu-2404",
            "other",
        ),
        (
            "schedule",
            "ci.yml",
            "build-test",
            "ubicloud-standard-4-ubuntu-2404",
            "other",
        ),
    ],
)
def test_cohorts_do_not_mix_writer_coverage_or_fork(
    attempt: tuple[dict, dict], case: tuple[str, str, str, str, str]
) -> None:
    """Keep execution paths separate even when runner shape matches."""
    run, jobs = attempt
    event, path, name, label, cohort = case
    run.update(event=event, path=path)
    jobs["jobs"][0].update(name=name, labels=[label])
    assert collect_attempt(run, jobs)[0]["cohort"] == cohort, (
        "Runner shape alone must not determine the execution cohort"
    )


def test_mixed_attempt_jobs_are_rejected(attempt: tuple[dict, dict]) -> None:
    """Do not attach an earlier attempt's logs or cost to a later attempt."""
    run, jobs = attempt
    jobs["jobs"][0]["run_attempt"] = 1
    with pytest.raises(ValueError, match="attempt mismatch"):
        collect_attempt(run, jobs)


@pytest.mark.parametrize("field", ["run_id", "run_attempt"])
@pytest.mark.parametrize("invalid_identity", [True, 1.0])
def test_equal_non_integer_job_provenance_is_rejected(
    attempt: tuple[dict, dict], field: str, *, invalid_identity: bool | float
) -> None:
    """Reject malformed provenance even when Python compares it equal to one."""
    run, jobs = attempt
    run.update(id=1, run_attempt=1)
    jobs["jobs"][0].update(run_id=1, run_attempt=1)
    jobs["jobs"][0][field] = invalid_identity
    with pytest.raises(TypeError, match="invalid integer identity"):
        collect_attempt(run, jobs)


def test_incomplete_jobs_page_is_rejected(attempt: tuple[dict, dict]) -> None:
    """Aggregate consumption requires the complete jobs API response."""
    run, jobs = attempt
    jobs["total_count"] = 2
    with pytest.raises(ValueError, match="incomplete jobs"):
        collect_attempt(run, jobs)


@pytest.mark.parametrize("invalid_count", [True, 1.0])
def test_equal_non_integer_page_count_is_rejected(
    attempt: tuple[dict, dict], *, invalid_count: bool | float
) -> None:
    """Do not infer a complete jobs page from a malformed total count."""
    run, jobs = attempt
    jobs["total_count"] = invalid_count
    with pytest.raises(ValueError, match="incomplete jobs"):
        collect_attempt(run, jobs)


def test_duplicate_jobs_cannot_double_charge(attempt: tuple[dict, dict]) -> None:
    """A complete-looking page with duplicate IDs is still invalid evidence."""
    run, jobs = attempt
    jobs["jobs"].append(jobs["jobs"][0].copy())
    jobs["total_count"] = 2
    with pytest.raises(ValueError, match="duplicate job identity"):
        collect_attempt(run, jobs)


def test_cancelled_attempt_retains_observed_cost(attempt: tuple[dict, dict]) -> None:
    """Retain failed work while refusing a complete-cost claim for unknown jobs."""
    run, jobs = attempt
    jobs["jobs"][0]["conclusion"] = "cancelled"
    jobs["jobs"].append(jobs["jobs"][0] | {"id": 457, "completed_at": None})
    jobs["total_count"] = 2
    record = collect_attempt(run, jobs)[0]
    assert record["outcome"] == "cancelled", (
        "Cancellation must remain visible in reliability accounting"
    )
    assert record["attempt_runner_seconds"] is None, (
        "An unfinished job prevents a complete-cost claim"
    )
    assert record["attempt_observed_runner_seconds"] == 10, (
        "Completed work must remain in the observed cost lower bound"
    )
    assert record["attempt_unknown_duration_jobs"] == 1, (
        "Unknown durations must be counted explicitly"
    )


def test_tested_checkout_sha_is_distinct_from_source(
    attempt: tuple[dict, dict],
) -> None:
    """Keep PR source provenance separate from the actual checked-out merge."""
    run, jobs = attempt
    log = "[command]/usr/bin/git log -1 --format=%H\n" + "b" * 40
    record = collect_attempt(run, jobs, {"logs": {"456": log}})[0]
    assert record["source_sha"] == "a" * 40, "Source provenance must retain the PR head"
    assert record["tested_merge_sha"] == "b" * 40, (
        "Checkout provenance must retain the tested merge"
    )


def test_cli_reads_saved_files_without_network(
    attempt: tuple[dict, dict], tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Exercise the actual CLI's flattened paths and JSONL delivery boundary."""
    run, jobs = attempt
    run_path, jobs_path = tmp_path / "run.json", tmp_path / "jobs.json"
    log_dir, telemetry_dir = tmp_path / "logs", tmp_path / "telemetry"
    log_dir.mkdir()
    telemetry_dir.mkdir()
    run_path.write_text(json.dumps(run), encoding="utf-8")
    jobs_path.write_text(json.dumps(jobs), encoding="utf-8")
    arguments = [
        "--run",
        str(run_path),
        "--jobs",
        str(jobs_path),
        "--log-dir",
        str(log_dir),
        "--telemetry-dir",
        str(telemetry_dir),
        "--variant",
        "public_api_prep",
    ]
    app(arguments, result_action="return_value")
    record = json.loads(capsys.readouterr().out)
    assert record["variant"] == "public_api_prep", (
        "The CLI must preserve the selected variant"
    )
    assert record["run_attempt"] == 2, "The CLI must retain the saved run attempt"
    assert record["whole_job_seconds"] == 10, "The CLI must preserve whole-job duration"
    assert record["log_available"] is False, "Missing logs must remain unavailable"


@pytest.mark.parametrize(
    "shape",
    [
        "ubicloud-standard-8-ubuntu-2404",
        "ubicloud-standard-4-arm-ubuntu-2404",
    ],
)
def test_unlisted_ubicloud_shapes_retain_runner_provenance(
    attempt: tuple[dict, dict], shape: str
) -> None:
    """Preserve capacity labels and distinct cost for other paid runner shapes."""
    run, jobs = attempt
    jobs["jobs"][0]["labels"] = [shape]

    record = collect_attempt(run, jobs)[0]

    assert record["runner_labels"] == [shape], (
        "Every recognized paid shape must retain its runner provenance"
    )
    assert record["attempt_ubicloud_seconds_by_shape"] == {shape: 10}, (
        "Each recognized paid shape must retain a separate cost proxy"
    )


def test_cost_preserves_runner_shapes(attempt: tuple[dict, dict]) -> None:
    """Separate the existing Kani and Linux gate shapes in the cost proxy."""
    run, jobs = attempt
    smaller = "ubicloud-standard-2-ubuntu-2404"
    larger = "ubicloud-standard-4-ubuntu-2404"
    jobs["jobs"].append(jobs["jobs"][0] | {"id": 457, "labels": [smaller]})
    jobs["total_count"] = 2
    records = collect_attempt(run, jobs)
    assert records[0]["attempt_ubicloud_seconds_by_shape"] == {
        smaller: 10,
        larger: 10,
    }, "Different Ubicloud shapes must remain separate cost proxies"
    assert records[1]["runner_labels"] == [smaller], (
        "Runner provenance must retain the smaller paid shape"
    )


def test_non_trunk_push_is_not_a_trusted_writer(attempt: tuple[dict, dict]) -> None:
    """Require the trusted branch before assigning a trunk cohort."""
    run, jobs = attempt
    run.update(event="push", head_branch="feature")
    assert collect_attempt(run, jobs)[0]["cohort"] == "other", (
        "A non-trunk push must never become a trusted writer observation"
    )
