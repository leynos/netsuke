"""Exercise bounded release-admission metrics through fake command adapters."""

import math
from pathlib import Path

import pytest
from release_admission_test_support import (
    CANARY_BY_OPERATION,
    METRICS_VALIDATOR,
    _run_gate,
    expected_gate_labels,
    expected_operation_labels,
)

FRESH_OBSERVATION_GATE_OUTPUTS = {
    "gate-outcome": "failure",
    "gate-error-category": "missing_evidence",
}
FRESH_OBSERVATION_GATE_RECORD = {
    "name": "netsuke_release_admission_gate_total",
    "labels": expected_gate_labels("failure", "missing_evidence"),
    "value": 1,
}
EXPECTED_TRACE_EVENTS = {
    "operation_complete",
    "gate_complete",
    "workflow_output_delivery",
    "trace_delivery",
}
TRACE_DELIVERY_FAILURE = {
    "event": "trace_delivery",
    "operation": "verify_evidence",
    "outcome": "failure",
    "error_category": "unknown",
    "duration_seconds": 0,
}


def operation_records(
    metrics: list[dict[str, object]], operation: str
) -> list[dict[str, object]]:
    """Return operation-counter records emitted for one fixed operation.

    Parameters
    ----------
    metrics
        Parsed release-admission metric records.
    operation
        One fixed operation from ``CANARY_BY_OPERATION``.

    Returns
    -------
    list[dict[str, object]]
        Counter records whose bounded ``operation`` label matches the request.
    """
    return [
        record
        for record in metrics
        if record["name"] == "netsuke_release_admission_operation_total"
        and isinstance(record["labels"], dict)
        and record["labels"].get("operation") == operation
    ]


def operation_duration(metrics: list[dict[str, object]], operation: str) -> float:
    """Return one finite duration observation for a fixed operation.

    Parameters
    ----------
    metrics
        Parsed release-admission metric records.
    operation
        One fixed operation from ``CANARY_BY_OPERATION``.

    Returns
    -------
    float
        The operation duration in seconds.
    """
    value = next(
        record["value"]
        for record in metrics
        if record["name"] == "netsuke_release_admission_operation_duration_seconds"
        and record["labels"] == {"operation": operation}
    )
    assert isinstance(value, int | float), (
        "duration records must contain finite numeric values"
    )
    assert math.isfinite(value), "duration records must contain finite numeric values"
    return float(value)


def test_gate_observes_synthetic_fresh_evidence_without_blocking(
    tmp_path: Path,
) -> None:
    """Verify synthetic fresh evidence remains an observed gate failure.

    Parameters
    ----------
    tmp_path
        Isolated fake-command and output directory.
    """
    result, metrics, traces, calls, outputs = _run_gate(
        Path(tmp_path), evidence_state="fresh"
    )

    assert result.returncode == 0, result.stderr
    METRICS_VALIDATOR.validate_metrics(metrics)
    METRICS_VALIDATOR.validate_traces(traces)
    assert {call["command"] for call in calls} == {"gh", "git"}, (
        "the production API and Git adapter contracts must both execute"
    )
    assert len(metrics) == 11, "five operations need counters and durations plus gate"
    duration_operations = {
        record["labels"]["operation"]
        for record in metrics
        if record["name"] == "netsuke_release_admission_operation_duration_seconds"
        and isinstance(record["labels"], dict)
    }
    assert duration_operations == CANARY_BY_OPERATION.keys(), (
        "every fixed operation must emit its bounded duration"
    )
    for operation, canary in CANARY_BY_OPERATION.items():
        records = operation_records(metrics, operation)
        assert len(records) == 1, f"{operation} must emit exactly one counter"
        outcome = "failure" if operation == "verify_evidence" else "success"
        error_category = "missing_evidence" if outcome == "failure" else "none"
        assert records[0]["labels"] == expected_operation_labels(
            canary, operation, outcome, error_category
        ), f"{operation} must retain its bounded outcome and error category"
    assert metrics[-1] == FRESH_OBSERVATION_GATE_RECORD, (
        "synthetic freshness must retain the producer-backed evidence failure"
    )
    assert {
        name: outputs[name] for name in FRESH_OBSERVATION_GATE_OUTPUTS
    } == FRESH_OBSERVATION_GATE_OUTPUTS, "observation must publish the gate result"
    expected_metrics_file = str(Path(tmp_path) / "release-admission-metrics.jsonl")
    assert outputs["metrics-file"] == expected_metrics_file, (
        "workflow output must identify the metric artefact"
    )
    assert outputs["trace-file"] == str(
        Path(tmp_path) / "release-admission-traces.jsonl"
    ), "workflow output must identify the trace artefact"
    assert {trace["event"] for trace in traces} == EXPECTED_TRACE_EVENTS, (
        "traces must include operation, gate, output, and delivery boundaries"
    )


def test_validator_rejects_non_finite_metric_values() -> None:
    """Verify the JSON contract rejects non-finite metric observations."""
    with pytest.raises(ValueError, match="release-admission metric records"):
        METRICS_VALIDATOR.parse_metrics(['{"value": Infinity}'])


def test_trace_sink_failure_preserves_gate_and_reports_recovery(
    tmp_path: Path,
) -> None:
    """Verify the trace sink is fail-open and reports bounded delivery failure.

    Parameters
    ----------
    tmp_path
        Isolated fake-command and sink directory.
    """
    working_directory = Path(tmp_path)
    trace_sink = working_directory / "flaky-trace-sink"
    state_file = working_directory / "trace-sink-state"
    trace_sink.write_text(
        "#!/usr/bin/env bash\n"
        "set -euo pipefail\n"
        "if [[ ! -e $NETSUKE_TRACE_SINK_STATE ]]; then\n"
        "  : >$NETSUKE_TRACE_SINK_STATE\n"
        "  exit 1\n"
        "fi\n"
        "cat >>$1\n",
        encoding="utf-8",
    )
    trace_sink.chmod(0o755)

    result, metrics, traces, _, outputs = _run_gate(
        working_directory / "run",
        evidence_state="fresh",
        extra_environment={
            "NETSUKE_RELEASE_ADMISSION_TRACE_SINK": str(trace_sink),
            "NETSUKE_TRACE_SINK_STATE": str(state_file),
        },
    )

    assert result.returncode == 0, result.stderr
    METRICS_VALIDATOR.validate_metrics(metrics)
    METRICS_VALIDATOR.validate_traces(traces)
    assert outputs["gate-outcome"] == "failure", (
        "a trace failure must not replace the observed admission result"
    )
    assert traces[-1] == TRACE_DELIVERY_FAILURE, (
        "a recovered trace sink must record its fixed delivery failure"
    )
