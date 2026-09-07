"""Provide fixed-record assertions shared by release-admission runtime tests."""

import math

OPERATION_SEQUENCE = (
    "resolve_tag_commit",
    "fetch_candidate_revision",
    "fetch_workflow_run",
    "check_scan_freshness",
    "verify_evidence",
)


def assert_failure_trace_sequence(
    traces: list[dict[str, object]], operation: str, error_category: str
) -> None:
    """Assert the complete bounded trace hand-off for one failed operation.

    Parameters
    ----------
    traces
        Parsed trace records in emission order.
    operation
        Fixed operation that stopped admission.
    error_category
        Fixed category assigned to ``operation``.

    Notes
    -----
    Contract invariant: successful predecessors, the failed operation, gate
    completion, workflow-output delivery, and trace delivery are all present.
    """
    failure_index = OPERATION_SEQUENCE.index(operation)
    expected = [
        ("operation_complete", predecessor, "success", "none")
        for predecessor in OPERATION_SEQUENCE[:failure_index]
    ] + [
        ("operation_complete", operation, "failure", error_category),
        ("gate_complete", "verify_evidence", "failure", error_category),
        ("workflow_output_delivery", "verify_evidence", "success", "none"),
        ("trace_delivery", "verify_evidence", "success", "none"),
    ]
    assert [
        tuple(
            trace[field]
            for field in ("event", "operation", "outcome", "error_category")
        )
        for trace in traces
    ] == expected, "failure traces must retain each bounded hand-off"


def operation_records(
    metrics: list[dict[str, object]], operation: str
) -> list[dict[str, object]]:
    """Return counter records for one fixed operation.

    Parameters
    ----------
    metrics
        Parsed release-admission metric records.
    operation
        Fixed operation name.

    Returns
    -------
    list[dict[str, object]]
        Records whose bounded operation label matches ``operation``.
    """
    return [
        record
        for record in metrics
        if record["name"] == "netsuke_release_admission_operation_total"
        and isinstance(record["labels"], dict)
        and record["labels"].get("operation") == operation
    ]


def operation_duration(metrics: list[dict[str, object]], operation: str) -> float:
    """Return a finite duration observation for a fixed operation.

    Parameters
    ----------
    metrics
        Parsed release-admission metric records.
    operation
        Fixed operation name.

    Returns
    -------
    float
        The finite operation duration in seconds.

    Notes
    -----
    Contract invariants: reject missing, non-numeric, and non-finite values.
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
