"""Exercise bounded release-admission metrics on controlled failure paths."""

import typing as typ

import pytest
from release_admission_test_support import (
    CANARY_BY_OPERATION,
    GITHUB_REPOSITORY,
    METRICS_VALIDATOR,
    _run_gate,
    assert_failure_trace_sequence,
    expected_gate_labels,
    expected_operation_labels,
    operation_duration,
    operation_records,
)

if typ.TYPE_CHECKING:
    from pathlib import Path

INVALID_CONFIGURATION_METRICS = [
    {
        "name": "netsuke_release_admission_gate_total",
        "labels": {"outcome": "failure", "error_category": "unknown"},
        "value": 1,
    }
]
INVALID_CONFIGURATION_TRACE_SIGNATURES = [
    ("gate_complete", "verify_evidence", "failure", "unknown"),
    ("workflow_output_delivery", "verify_evidence", "success", "none"),
    ("trace_delivery", "verify_evidence", "success", "none"),
]


def _trace_signature(trace: dict[str, object]) -> tuple[object, ...]:
    """Return the bounded fields that identify one trace record.

    Parameters
    ----------
    trace
        Decoded trace record with the fixed admission schema.

    Returns
    -------
    tuple[object, ...]
        Event, operation, outcome, and error category in schema order.

    Notes
    -----
    Contract invariant: only bounded fields participate in sequence assertions.
    """
    return tuple(
        trace[field] for field in ("event", "operation", "outcome", "error_category")
    )


def _assert_malformed_revision_is_a_bounded_mismatch(
    metrics: list[dict[str, object]],
    traces: list[dict[str, object]],
    outputs: dict[str, str],
    revision: str,
) -> None:
    """Assert malformed revisions retain only fixed diagnostic values.

    Parameters
    ----------
    metrics
        Metric records emitted by the gate subprocess.
    traces
        Trace records emitted by the gate subprocess.
    outputs
        GitHub workflow outputs emitted by the gate subprocess.
    revision
        Malformed revision used by the gate subprocess.
    """
    METRICS_VALIDATOR.validate_metrics(metrics)
    METRICS_VALIDATOR.validate_traces(traces)

    operation_metric = operation_records(metrics, "resolve_tag_commit")[-1]
    assert operation_metric["labels"] == expected_operation_labels(
        CANARY_BY_OPERATION["resolve_tag_commit"],
        "resolve_tag_commit",
        "failure",
        "mismatch",
    ), "malformed revisions must use the fixed commit-resolution mismatch labels"
    gate_labels = metrics[-1]["labels"]
    assert gate_labels == expected_gate_labels("failure", "mismatch"), (
        "the gate metric must retain the commit-resolution mismatch category"
    )
    assert outputs["gate-outcome"] == "failure", (
        "malformed revisions must publish a failed gate outcome"
    )
    assert (
        outputs["gate-error-category"]
        == typ.cast("dict[str, str]", gate_labels)["error_category"]
        == "mismatch"
    ), "workflow outputs must retain the gate metric's mismatch category"
    assert_failure_trace_sequence(traces, "resolve_tag_commit", "mismatch")

    assert all(
        forbidden_revision not in value
        for record in metrics
        for value in typ.cast("dict[str, str]", record["labels"]).values()
        for forbidden_revision in (revision, revision.rstrip("\n"))
        if forbidden_revision
    ), "malformed revisions must never become metric label values"


def _assert_malformed_revision_stops_followup_requests(
    calls: list[dict[str, object]], revision: str
) -> None:
    """Assert a malformed revision stops before fetch and workflow lookup.

    Parameters
    ----------
    calls
        Recorded fake-command and GitHub calls from the gate subprocess.
    revision
        Malformed revision used by the gate subprocess.
    """
    github_calls = [call for call in calls if call["command"] == "gh"]
    assert len(github_calls) == 1, (
        f"only commit resolution may cross GitHub; recorded calls: {calls!r}"
    )
    assert github_calls[0]["arguments"] == [
        "api",
        f"repos/{GITHUB_REPOSITORY}/commits/{revision}",
        "--jq",
        ".sha",
    ], "the sole GitHub call must resolve the malformed revision"
    assert not any(
        call["command"] == "git"
        and typ.cast("list[str]", call["arguments"])[0:1] == ["fetch"]
        for call in calls
    ), "a mismatched revision must not start Git fetch"
    assert not any(
        any(
            "/actions/runs?" in argument
            for argument in typ.cast("list[str]", call["arguments"])
        )
        for call in github_calls
    ), "a mismatched revision must not start workflow-run lookup"


@pytest.mark.parametrize(
    ("revision", "mode"),
    [
        pytest.param("a" * 40 + "\n", "enforcement", id="sha-newline-enforcement"),
        pytest.param("\n", "enforcement", id="newline-enforcement"),
        pytest.param("a" * 40 + "\n", "observation", id="sha-newline-observation"),
        pytest.param("\n", "observation", id="newline-observation"),
    ],
)
def test_malformed_revision_fails_as_mismatch_before_followup_requests(
    tmp_path: Path,
    revision: str,
    mode: typ.Literal["enforcement", "observation"],
) -> None:
    """Reject newline revisions before fetching or looking up workflow runs.

    Parameters
    ----------
    tmp_path
        Isolated fake-command and output directory.
    revision
        Malformed SHA text whose trailing newline is removed from command output.
    mode
        Whether the gate runs in enforcement or observation mode.
    """
    enforce = mode == "enforcement"
    result, metrics, traces, calls, outputs = _run_gate(
        tmp_path,
        evidence_state="fresh",
        extra_environment={
            "GITHUB_SHA": revision,
            "NETSUKE_RELEASE_ADMISSION_ENFORCE": str(enforce).lower(),
        },
    )

    assert result.returncode == (1 if enforce else 0), (
        "enforcement must reject the malformed revision while observation "
        "retains diagnostics"
    )
    _assert_malformed_revision_is_a_bounded_mismatch(metrics, traces, outputs, revision)
    _assert_malformed_revision_stops_followup_requests(calls, revision)


def test_default_observation_retains_missing_evidence_metrics(tmp_path: Path) -> None:
    """Verify missing evidence is observed without failing the scaffold workflow.

    Parameters
    ----------
    tmp_path
        Isolated fake-command and output directory.

    Notes
    -----
    The default must mirror the current workflow's lack of an evidence producer.
    """
    result, metrics, traces, _, outputs = _run_gate(tmp_path)

    assert result.returncode == 0, result.stderr
    METRICS_VALIDATOR.validate_metrics(metrics)
    METRICS_VALIDATOR.validate_traces(traces)
    record = operation_records(metrics, "check_scan_freshness")[-1]
    assert record["labels"] == expected_operation_labels(
        CANARY_BY_OPERATION["check_scan_freshness"],
        "check_scan_freshness",
        "failure",
        "missing_evidence",
    ), "observation must retain the missing-evidence operation result"
    assert metrics[-1]["labels"] == expected_gate_labels(
        "failure", "missing_evidence"
    ), "observation must retain the missing-evidence gate result"
    assert outputs["gate-outcome"] == "failure", (
        "observation must publish the failed gate outcome"
    )
    assert outputs["gate-error-category"] == "missing_evidence", (
        "observation must publish the fixed missing-evidence category"
    )


def test_operation_durations_measure_controlled_delay(tmp_path: Path) -> None:
    """Verify operation durations remain finite and measure command latency.

    Parameters
    ----------
    tmp_path
        Isolated fake-command and output directory.

    Notes
    -----
    A fixed workflow-run delay must lengthen only that operation's measurement.
    """
    _, metrics, _, _, _ = _run_gate(tmp_path, evidence_state="fresh")
    assert all(
        operation_duration(metrics, operation) > 0 for operation in CANARY_BY_OPERATION
    ), "every executed operation must record a finite positive duration"

    _, delayed_metrics, _, _, _ = _run_gate(
        tmp_path / "delayed",
        evidence_state="fresh",
        extra_environment={"NETSUKE_FAKE_GH_WORKFLOW_DELAY_SECONDS": "1"},
    )
    assert operation_duration(
        delayed_metrics, "fetch_workflow_run"
    ) > operation_duration(metrics, "fetch_workflow_run"), (
        "operation duration must increase when its bounded command is delayed"
    )


@pytest.mark.parametrize(
    ("setting", "value"),
    [
        pytest.param(
            "NETSUKE_RELEASE_ADMISSION_OPERATION_TIMEOUT_SECONDS",
            "0",
            id="timeout-seconds-0",
        ),
        pytest.param(
            "NETSUKE_RELEASE_ADMISSION_OPERATION_TIMEOUT_SECONDS",
            "301",
            id="timeout-seconds-301",
        ),
        pytest.param(
            "NETSUKE_RELEASE_ADMISSION_OPERATION_TIMEOUT_SECONDS",
            "not-a-number",
            id="timeout-seconds-not-a-number",
        ),
        pytest.param("NETSUKE_RELEASE_ADMISSION_ENFORCE", "", id="enforce-empty"),
        pytest.param("NETSUKE_RELEASE_ADMISSION_ENFORCE", "False", id="enforce-False"),
        pytest.param(
            "NETSUKE_RELEASE_ADMISSION_ENFORCE", "observe", id="enforce-observe"
        ),
    ],
)
def test_invalid_configuration_fails_before_running_admission_operations(
    tmp_path: Path,
    setting: str,
    value: str,
) -> None:
    """Verify invalid configuration fails before any external command runs.

    Parameters
    ----------
    tmp_path
        Isolated fake-command and output directory.
    setting
        Closed configuration input that receives an invalid value.
    value
        Out-of-contract configuration value supplied to the shell boundary.

    Notes
    -----
    Early validation must still emit valid gate metrics, traces, and workflow
    outputs while preventing every external admission operation.
    """
    result, metrics, traces, calls, outputs = _run_gate(
        tmp_path,
        evidence_state="fresh",
        extra_environment={setting: value},
    )

    assert result.returncode != 0, "invalid configuration must fail closed"
    assert calls == [], "invalid configuration must prevent API and Git calls"
    METRICS_VALIDATOR.validate_metrics(metrics)
    METRICS_VALIDATOR.validate_traces(traces)
    assert metrics == INVALID_CONFIGURATION_METRICS, (
        "invalid configuration must emit only the fixed failure gate metric"
    )
    assert outputs["gate-outcome"] == "failure", (
        "invalid configuration must publish failure"
    )
    assert outputs["gate-error-category"] == "unknown", (
        "invalid configuration must publish the fixed unknown category"
    )
    assert [_trace_signature(trace) for trace in traces] == (
        INVALID_CONFIGURATION_TRACE_SIGNATURES
    ), "early configuration failure must retain the bounded trace hand-off sequence"
