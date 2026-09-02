"""Exercise bounded release-admission metrics on controlled failure paths."""

import typing as typ

import pytest
from test_release_admission_metrics import (
    CANARY_BY_OPERATION,
    METRICS_VALIDATOR,
    FailureCase,
    _run_gate,
    expected_gate_labels,
    expected_operation_labels,
    operation_records,
)

if typ.TYPE_CHECKING:
    from pathlib import Path


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
                {"NETSUKE_FAKE_WORKFLOW_RUN_ID": ""},
                "verify_evidence",
                "missing_evidence",
            ),
            id="missing-workflow-run",
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
    result, metrics, _, outputs = _run_gate(
        tmp_path,
        evidence_state=case.evidence_state,
        extra_environment=case.extra_environment,
    )

    assert result.returncode != 0, "a failed admission operation must block the gate"
    METRICS_VALIDATOR.validate_metrics(metrics)
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
    if case.error_category == "timeout":
        duration = next(
            record["value"]
            for record in metrics
            if record["name"] == "netsuke_release_admission_operation_duration_seconds"
            and record["labels"] == {"operation": case.operation}
        )
        assert isinstance(duration, int | float), (
            "timed-out operations must record numeric durations"
        )
        assert duration > 0, (
            "timed-out operations must retain a positive measured duration"
        )
