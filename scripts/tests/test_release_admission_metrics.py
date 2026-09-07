"""Exercise bounded release-admission metrics through fake command adapters."""

import typing as typ
from pathlib import Path

import pytest
from release_admission_test_fakes import (
    FAKE_ADAPTER_PREAMBLE,
    FAKE_GH_BEHAVIOUR,
    FAKE_GIT_BEHAVIOUR,
)
from release_admission_test_support import (
    CANARY_BY_OPERATION,
    METRICS_VALIDATOR,
    REVISION,
    _run_gate,
    expected_gate_labels,
    expected_operation_labels,
    operation_duration,
    operation_records,
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


def _write_recording_adapter(directory: Path, name: str, behaviour: str) -> Path:
    """Write one explicit adapter that records its native invocation."""
    adapter = directory / name
    adapter.write_text(
        FAKE_ADAPTER_PREAMBLE.replace("ADAPTER_NAME", name) + behaviour,
        encoding="utf-8",
    )
    adapter.chmod(0o755)
    return adapter


def _trace_signature(trace: dict[str, object]) -> tuple[object, ...]:
    """Return the bounded fields that identify one trace record."""
    return tuple(
        trace[field] for field in ("event", "operation", "outcome", "error_category")
    )


def _write_recording_adapters(directory: Path) -> dict[str, Path]:
    """Write the complete set of independently selectable admission adapters."""
    directory.mkdir()
    sink_behaviour = 'cat >>"$1"\n'
    return {
        "NETSUKE_RELEASE_ADMISSION_GH_ADAPTER": _write_recording_adapter(
            directory, "gh-adapter", FAKE_GH_BEHAVIOUR
        ),
        "NETSUKE_RELEASE_ADMISSION_GIT_ADAPTER": _write_recording_adapter(
            directory, "git-adapter", FAKE_GIT_BEHAVIOUR
        ),
        "NETSUKE_RELEASE_ADMISSION_CLOCK_ADAPTER": _write_recording_adapter(
            directory, "clock-adapter", "printf '1\\n'\n"
        ),
        "NETSUKE_RELEASE_ADMISSION_METRICS_SINK": _write_recording_adapter(
            directory, "metrics-sink", sink_behaviour
        ),
        "NETSUKE_RELEASE_ADMISSION_OUTPUT_SINK": _write_recording_adapter(
            directory, "output-sink", sink_behaviour
        ),
        "NETSUKE_RELEASE_ADMISSION_TRACE_SINK": _write_recording_adapter(
            directory, "trace-sink", sink_behaviour
        ),
    }


def _write_failing_clock_adapter(tmp_path: Path) -> Path:
    """Write a clock adapter that fails on one configured invocation."""
    adapter = tmp_path / "clock-adapter"
    adapter.write_text(
        "#!/usr/bin/env bash\n"
        "set -euo pipefail\n"
        "count=0\n"
        "if [[ -e $NETSUKE_FAKE_CLOCK_STATE ]]; then\n"
        '  count=$(<"$NETSUKE_FAKE_CLOCK_STATE")\n'
        "fi\n"
        "count=$((count + 1))\n"
        'printf \'%s\' "$count" >"$NETSUKE_FAKE_CLOCK_STATE"\n'
        "if [[ $count == $NETSUKE_FAKE_CLOCK_FAILURE ]]; then exit 1; fi\n"
        "printf '%s\\n' \"$count\"\n",
        encoding="utf-8",
    )
    adapter.chmod(0o755)
    return adapter


def _assert_fresh_observation_metrics(metrics: list[dict[str, object]]) -> None:
    """Assert the fixed metric contract for synthetic fresh evidence."""
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


def _assert_fresh_observation_outputs(outputs: dict[str, str], tmp_path: Path) -> None:
    """Assert the fixed workflow-output contract for synthetic fresh evidence."""
    assert {
        name: outputs[name] for name in FRESH_OBSERVATION_GATE_OUTPUTS
    } == FRESH_OBSERVATION_GATE_OUTPUTS, "observation must publish the gate result"
    expected_metrics_file = str(tmp_path / "release-admission-metrics.jsonl")
    assert outputs["metrics-file"] == expected_metrics_file, (
        "workflow output must identify the metric artefact"
    )
    assert outputs["trace-file"] == str(tmp_path / "release-admission-traces.jsonl"), (
        "workflow output must identify the trace artefact"
    )


def _assert_fresh_observation_traces(traces: list[dict[str, object]]) -> None:
    """Assert the fixed trace sequence for synthetic fresh evidence."""
    assert {trace["event"] for trace in traces} == EXPECTED_TRACE_EVENTS, (
        "traces must include operation, gate, output, and delivery boundaries"
    )
    assert [_trace_signature(trace) for trace in traces] == [
        ("operation_complete", operation, "success", "none")
        for operation in (
            "resolve_tag_commit",
            "fetch_candidate_revision",
            "fetch_workflow_run",
            "check_scan_freshness",
        )
    ] + [
        (
            "operation_complete",
            "verify_evidence",
            "failure",
            "missing_evidence",
        ),
        ("gate_complete", "verify_evidence", "failure", "missing_evidence"),
        ("workflow_output_delivery", "verify_evidence", "success", "none"),
        ("trace_delivery", "verify_evidence", "success", "none"),
    ], "successful operation sequence must retain all bounded trace hand-offs"


def test_gate_observes_synthetic_fresh_evidence_without_blocking(
    tmp_path: Path,
) -> None:
    """Verify synthetic fresh evidence remains an observed gate failure.

    Parameters
    ----------
    tmp_path
        Isolated fake-command and output directory.

    Notes
    -----
    Contract invariants: observation mode returns success while retaining the
    failed gate result and all bounded operation and trace records.
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
    _assert_fresh_observation_metrics(metrics)
    _assert_fresh_observation_outputs(outputs, Path(tmp_path))
    _assert_fresh_observation_traces(traces)


def _assert_sink_adapter_targets(
    calls_by_adapter: dict[str, list[dict[str, object]]],
    outputs: dict[str, str],
    run_directory: Path,
) -> None:
    """Assert each sink adapter writes only to its configured target."""
    expected_targets = {
        "metrics-sink": outputs["metrics-file"],
        "output-sink": str(run_directory / "github-output"),
        "trace-sink": outputs["trace-file"],
    }
    for adapter_name, target in expected_targets.items():
        assert calls_by_adapter[adapter_name], f"{adapter_name} must receive records"
        assert all(
            call["arguments"] == [target] for call in calls_by_adapter[adapter_name]
        ), f"{adapter_name} must receive only its configured output target"


def test_explicit_adapters_receive_native_boundary_contracts(tmp_path: Path) -> None:
    """Verify every documented adapter receives only its native contract.

    Parameters
    ----------
    tmp_path
        Isolated adapter, call-log, and JSONL output directory.

    Notes
    -----
    Contract invariants: API and Git adapters receive native command arguments;
    clock receives Python arguments; sinks receive only their configured file
    target and preserve bounded metric, trace, and workflow-output records.
    """
    result, metrics, traces, calls, outputs = _run_gate(
        tmp_path / "run",
        evidence_state="fresh",
        extra_environment={
            key: str(adapter)
            for key, adapter in _write_recording_adapters(tmp_path / "adapters").items()
        },
    )

    assert result.returncode == 0, result.stderr
    METRICS_VALIDATOR.validate_metrics(metrics)
    METRICS_VALIDATOR.validate_traces(traces)
    calls_by_adapter = {
        name: [call for call in calls if call["command"] == name]
        for name in (
            "gh-adapter",
            "git-adapter",
            "clock-adapter",
            "metrics-sink",
            "output-sink",
            "trace-sink",
        )
    }
    expected_github_calls = [
        ["api", f"repos/leynos/netsuke/commits/{REVISION}", "--jq", ".sha"],
        [
            "api",
            f"repos/leynos/netsuke/actions/runs?head_sha={REVISION}&per_page=1",
            "--jq",
            ".workflow_runs[0].id // empty",
        ],
    ]
    assert [call["arguments"] for call in calls_by_adapter["gh-adapter"]] == (
        expected_github_calls
    ), "GitHub adapter must receive both native admission queries"
    assert [call["arguments"] for call in calls_by_adapter["git-adapter"]] == [
        ["fetch", "--depth", "1", "--no-tags", "origin", "--", REVISION]
    ], "Git adapter must receive the bounded native fetch arguments"
    clock_arguments = [
        typ.cast("list[str]", call["arguments"])
        for call in calls_by_adapter["clock-adapter"]
    ]
    assert all(arguments[0] == "-c" for arguments in clock_arguments), (
        "clock adapter must receive the Python clock program argument"
    )
    _assert_sink_adapter_targets(calls_by_adapter, outputs, tmp_path / "run")


def test_validator_rejects_non_finite_metric_values() -> None:
    """Verify the JSON contract rejects non-finite metric observations.

    Notes
    -----
    Contract invariant: JSON ``Infinity`` is rejected before metric schema
    validation can accept the record.
    """
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

    Notes
    -----
    Contract invariants: trace-sink failure is fail-open for the gate, and a
    bounded delivery-failure record is emitted when the sink recovers.
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


@pytest.mark.parametrize("failure_read", ["1", "2"], ids=["start", "finish"])
def test_clock_failure_retains_bounded_operation_result(
    tmp_path: Path,
    failure_read: str,
) -> None:
    """Verify either clock read emits a bounded failed operation result.

    Parameters
    ----------
    tmp_path
        Isolated fake-command and output directory.
    failure_read
        The clock invocation that the adapter fails.

    Notes
    -----
    Contract invariants: either clock failure produces an unknown operation
    result, a zero duration fallback, and a failed gate output.
    """
    adapter = _write_failing_clock_adapter(tmp_path)
    result, metrics, traces, _, outputs = _run_gate(
        tmp_path / "run",
        evidence_state="fresh",
        extra_environment={
            "NETSUKE_RELEASE_ADMISSION_CLOCK_ADAPTER": str(adapter),
            "NETSUKE_FAKE_CLOCK_FAILURE": failure_read,
            "NETSUKE_FAKE_CLOCK_STATE": str(tmp_path / "clock-state"),
        },
    )

    assert result.returncode == 0, result.stderr
    METRICS_VALIDATOR.validate_metrics(metrics)
    METRICS_VALIDATOR.validate_traces(traces)
    operation = "resolve_tag_commit"
    assert operation_records(metrics, operation)[-1]["labels"] == (
        expected_operation_labels("none", operation, "failure", "unknown")
    ), "clock failure must retain a bounded operation failure"
    assert operation_duration(metrics, operation) == 0, (
        "clock failure must retain the defined zero-duration fallback"
    )
    assert outputs["gate-outcome"] == "failure", (
        "clock failure must reach the workflow output boundary"
    )
    assert outputs["gate-error-category"] == "unknown", (
        "clock failure must retain the bounded unknown category"
    )
