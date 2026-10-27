"""Specify the serialized step facts emitted by offline CI evidence collection."""

import json
from pathlib import Path

import pytest
from ci_test_evidence import collect_attempt


def test_collector_serializes_populated_step_facts(
    attempt: tuple[dict, dict],
) -> None:
    """Retain phase, normalized outcome, timestamps, and elapsed seconds."""
    run, jobs = attempt
    jobs["jobs"][0]["steps"] = [
        {
            "number": 3,
            "name": "Install Rust",
            "conclusion": "not-a-known-outcome",
            "started_at": "2026-10-12T00:00:00Z",
            "completed_at": "2026-10-12T00:00:02.500Z",
        }
    ]

    step = json.loads(json.dumps(collect_attempt(run, jobs)[0]))["steps"][0]
    snapshot = Path(__file__).parent / "snapshots" / "ci_test_evidence_steps.json"

    assert step["started_at"] == "2026-10-12T00:00:00Z", (
        "Serialized steps must retain valid start timestamps"
    )
    assert step["completed_at"] == "2026-10-12T00:00:02.500Z", (
        "Serialized steps must retain valid completion timestamps"
    )
    assert step["seconds"] == pytest.approx(2.5), (
        "Serialized steps must retain elapsed seconds"
    )
    stable_fields = {
        key: value
        for key, value in step.items()
        if key not in {"started_at", "completed_at", "seconds"}
    }

    assert [stable_fields] == json.loads(snapshot.read_text(encoding="utf-8")), (
        "Serialized steps must retain their stable normalized API facts"
    )


@pytest.mark.parametrize(
    "case",
    [
        (
            "malformed",
            "2026-10-12T00:00:02Z",
            None,
            "2026-10-12T00:00:02Z",
            None,
        ),
        (
            "2026-10-12T00:00:02Z",
            "2026-10-12T00:00:01Z",
            "2026-10-12T00:00:02Z",
            "2026-10-12T00:00:01Z",
            None,
        ),
        (
            "2026-10-12T00:00:00",
            "2026-10-12T00:00:02Z",
            None,
            "2026-10-12T00:00:02Z",
            None,
        ),
        (
            "2026-10-12T00:00:00Z",
            "2026-10-12T00:00:02",
            "2026-10-12T00:00:00Z",
            None,
            None,
        ),
    ],
    ids=["malformed", "reversed", "naive-start", "naive-end"],
)
def test_collector_keeps_invalid_step_times_unknown(
    attempt: tuple[dict, dict],
    case: tuple[str, str, str | None, str | None, float | None],
) -> None:
    """Do not turn malformed, reversed, or naive times into measurements."""
    started_at, completed_at, expected_start, expected_end, seconds = case
    run, jobs = attempt
    jobs["jobs"][0]["steps"] = [
        {
            "number": 1,
            "name": "Run tests",
            "conclusion": "success",
            "started_at": started_at,
            "completed_at": completed_at,
        }
    ]

    record = json.loads(json.dumps(collect_attempt(run, jobs)[0]))
    step = record["steps"][0]

    assert step["started_at"] == expected_start, (
        "Invalid step starts must remain unknown"
    )
    assert step["completed_at"] == expected_end, (
        "Invalid step completions must remain unknown"
    )
    assert step["seconds"] == seconds, (
        "Invalid or reversed step times must not become durations"
    )
