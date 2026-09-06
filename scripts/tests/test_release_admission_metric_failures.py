"""Exercise bounded release-admission metrics on controlled failure paths."""

import typing as typ

import pytest
from release_admission_test_support import (
    CANARY_BY_OPERATION,
    FailureCase,
    METRICS_VALIDATOR,
    _run_gate,
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


@pytest.mark.parametrize(
    "case",
    [
        pytest.param(
            FailureCase(
                "fresh",
                {"NETSUKE_FAKE_GH_FAILURE": "true"},
                "resolve_tag_commit",
                "api_error",
            ),
            id="api-error",
        ),
        pytest.param(
            FailureCase(
                "fresh",
                {"NETSUKE_FAKE_RESOLVED_REVISION": "b" * 40},
                "resolve_tag_commit",
                "mismatch",
            ),
            id="candidate-mismatch",
        ),
        pytest.param(
            FailureCase(
                "fresh",
                {"NETSUKE_FAKE_GIT_FAILURE": "true"},
                "fetch_candidate_revision",
                "fetch_error",
            ),
            id="fetch-error",
        ),
        pytest.param(
            FailureCase("stale", {}, "check_scan_freshness", "stale_evidence"),
            id="stale-evidence",
        ),
        pytest.param(
            FailureCase("missing", {}, "check_scan_freshness", "missing_evidence"),
            id="missing-evidence",
        ),
        pytest.param(
            FailureCase("unexpected", {}, "check_scan_freshness", "unknown"),
            id="unknown-evidence",
        ),
        pytest.param(
            FailureCase(
                "fresh",
                {},
                "verify_evidence",
                "missing_evidence",
            ),
            id="enforcement-rejects-environment-freshness",
        ),
        pytest.param(
            FailureCase(
                "fresh",
                {
                    "NETSUKE_FAKE_GH_DELAY_SECONDS": "2",
                    "NETSUKE_RELEASE_ADMISSION_OPERATION_TIMEOUT_SECONDS": "1",
                },
                "resolve_tag_commit",
                "timeout",
            ),
            id="operation-timeout",
        ),
        pytest.param(
            FailureCase(
                "fresh",
                {
                    "NETSUKE_FAKE_GH_IGNORE_TERM": "true",
                    "NETSUKE_RELEASE_ADMISSION_OPERATION_TIMEOUT_SECONDS": "1",
                },
                "resolve_tag_commit",
                "timeout",
            ),
            id="term-ignoring-timeout",
        ),
        pytest.param(
            FailureCase(
                "fresh",
                {"NETSUKE_FAKE_WORKFLOW_RUN_ID": ""},
                "verify_evidence",
                "missing_evidence",
                enforce=False,
            ),
            id="missing-workflow-run-observation",
        ),
        pytest.param(
            FailureCase(
                "fresh",
                {"NETSUKE_FAKE_GH_WORKFLOW_FAILURE": "true"},
                "fetch_workflow_run",
                "api_error",
            ),
            id="workflow-run-api-error-enforcement",
        ),
        pytest.param(
            FailureCase(
                "fresh",
                {"NETSUKE_FAKE_GH_WORKFLOW_FAILURE": "true"},
                "fetch_workflow_run",
                "api_error",
                enforce=False,
            ),
            id="workflow-run-api-error-observation",
        ),
        pytest.param(
            FailureCase(
                "fresh",
                {
                    "NETSUKE_FAKE_GH_WORKFLOW_DELAY_SECONDS": "2",
                    "NETSUKE_RELEASE_ADMISSION_OPERATION_TIMEOUT_SECONDS": "1",
                },
                "fetch_workflow_run",
                "timeout",
            ),
            id="workflow-run-timeout-enforcement",
        ),
        pytest.param(
            FailureCase(
                "fresh",
                {
                    "NETSUKE_FAKE_GH_WORKFLOW_DELAY_SECONDS": "2",
                    "NETSUKE_RELEASE_ADMISSION_OPERATION_TIMEOUT_SECONDS": "1",
                },
                "fetch_workflow_run",
                "timeout",
                enforce=False,
            ),
            id="workflow-run-timeout-observation",
        ),
    ],
)
def test_gate_emits_fixed_categories_for_failure_paths(
    tmp_path: Path,
    case: FailureCase,
) -> None:
    """Verify every controlled failure retains a bounded metric category.

    Parameters
    ----------
    tmp_path
        Isolated fake-command and output directory.
    case
        One failure input and its documented fixed category.

    Notes
    -----
    Every failure must emit operation, gate, and workflow-output results before
    the admission script exits unsuccessfully.
    """
    result, metrics, traces, _, outputs = _run_gate(
        tmp_path,
        evidence_state=case.evidence_state,
        extra_environment={
            "NETSUKE_RELEASE_ADMISSION_ENFORCE": str(case.enforce).lower(),
            **case.extra_environment,
        },
    )

    assert result.returncode == (1 if case.enforce else 0), (
        "enforcement must fail closed while observation must retain diagnostics"
    )
    METRICS_VALIDATOR.validate_metrics(metrics)
    METRICS_VALIDATOR.validate_traces(traces)
    record = operation_records(metrics, case.operation)[-1]
    assert record["labels"] == expected_operation_labels(
        CANARY_BY_OPERATION[case.operation],
        case.operation,
        "failure",
        case.error_category,
    ), f"{case.operation} must retain its fixed error category"
    assert metrics[-1]["labels"] == expected_gate_labels(
        "failure", case.error_category
    ), "the gate must retain the operation's error category"
    assert outputs["gate-outcome"] == "failure", (
        "failed operations must reach the workflow summary output"
    )
    assert outputs["gate-error-category"] == case.error_category, (
        "failed operations must retain their bounded category in workflow output"
    )
    if case.extra_environment.get("NETSUKE_FAKE_WORKFLOW_RUN_ID") == "":
        workflow_run_record = operation_records(metrics, "fetch_workflow_run")[-1]
        assert workflow_run_record["labels"] == expected_operation_labels(
            CANARY_BY_OPERATION["fetch_workflow_run"],
            "fetch_workflow_run",
            "success",
            "none",
        ), "an empty run identifier must reach evidence verification"
    if case.error_category == "timeout":
        assert operation_duration(metrics, case.operation) > 0, (
            "timed-out operations must retain a positive measured duration"
        )


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


@pytest.mark.parametrize("timeout_value", ["0", "301", "not-a-number"])
def test_invalid_timeout_fails_before_running_admission_operations(
    tmp_path: Path,
    timeout_value: str,
) -> None:
    """Verify invalid timeout settings fail before any external command runs.

    Parameters
    ----------
    tmp_path
        Isolated fake-command and output directory.
    timeout_value
        Out-of-contract timeout configuration supplied to the shell boundary.

    Notes
    -----
    Early validation must still emit valid gate metrics and workflow outputs.
    """
    result, metrics, traces, calls, outputs = _run_gate(
        tmp_path,
        evidence_state="fresh",
        extra_environment={
            "NETSUKE_RELEASE_ADMISSION_OPERATION_TIMEOUT_SECONDS": timeout_value,
        },
    )

    assert result.returncode != 0, "invalid timeout configuration must fail closed"
    assert calls == [], "invalid timeout configuration must prevent API and Git calls"
    METRICS_VALIDATOR.validate_metrics(metrics)
    METRICS_VALIDATOR.validate_traces(traces)
    assert metrics == INVALID_CONFIGURATION_METRICS, (
        "invalid timeout configuration must emit only the fixed failure gate metric"
    )
    assert outputs["gate-outcome"] == "failure", (
        "invalid timeout configuration must publish failure"
    )
    assert outputs["gate-error-category"] == "unknown", (
        "invalid timeout configuration must publish the fixed unknown category"
    )


@pytest.mark.parametrize("enforcement_value", ["", "False", "observe"])
def test_invalid_enforcement_fails_before_running_admission_operations(
    tmp_path: Path,
    enforcement_value: str,
) -> None:
    """Verify invalid enforcement settings fail before any external command runs.

    Parameters
    ----------
    tmp_path
        Isolated fake-command and output directory.
    enforcement_value
        Out-of-contract enforcement mode supplied to the shell boundary.

    Notes
    -----
    Configuration validation must publish the same bounded failure contract as
    invalid timeout validation before an admission operation runs.
    """
    result, metrics, traces, calls, outputs = _run_gate(
        tmp_path,
        extra_environment={"NETSUKE_RELEASE_ADMISSION_ENFORCE": enforcement_value},
    )

    assert result.returncode != 0, "invalid enforcement configuration must fail closed"
    assert calls == [], (
        "invalid enforcement configuration must prevent API and Git calls"
    )
    METRICS_VALIDATOR.validate_metrics(metrics)
    METRICS_VALIDATOR.validate_traces(traces)
    assert metrics == INVALID_CONFIGURATION_METRICS, (
        "invalid enforcement configuration must emit only the fixed failure gate metric"
    )
    assert outputs["gate-outcome"] == "failure", (
        "invalid enforcement configuration must publish failure"
    )
    assert outputs["gate-error-category"] == "unknown", (
        "invalid enforcement configuration must publish the fixed unknown category"
    )
