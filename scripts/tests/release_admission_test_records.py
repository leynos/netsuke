"""Provide fixed-record assertions shared by release-admission runtime tests."""

import math
import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc

OPERATION_SEQUENCE = (
    "resolve_tag_commit",
    "fetch_candidate_revision",
    "fetch_workflow_run",
    "check_scan_freshness",
    "verify_evidence",
)


def assert_identifiers_excluded_from_values(
    values: cabc.Iterable[object], identifiers: set[str], message: str
) -> None:
    """Assert generated identifiers are absent from string values.

    Parameters
    ----------
    values
        Metric-label or trace-field values to inspect; non-string values are ignored.
    identifiers
        Identifiers that must not occur as substrings of any value.
    message
        Failure message prefix retained in the assertion diagnostic.

    Notes
    -----
    Use this helper for one metric-label or trace-field value collection at a
    time so callers can retain field-specific failure messages.

    Examples
    --------
    Ignore non-string values while checking labels and trace fields::

        assert_identifiers_excluded_from_values(
            ["fixed-label", 17], {"run-7"}, "identifier leaked"
        )
    """
    matches = [
        (identifier, value)
        for value in values
        if isinstance(value, str)
        for identifier in sorted(identifiers)
        if identifier in value
    ]
    assert not matches, f"{message}; matching identifier/value pairs: {matches!r}"


def assert_identifiers_excluded_from_records(
    metrics: cabc.Iterable[dict[str, object]],
    traces: cabc.Iterable[dict[str, object]],
    identifiers: set[str],
    subject: str,
) -> None:
    """Assert identifiers are absent from metric labels and trace fields.

    Parameters
    ----------
    metrics
        Parsed release-admission metric records to inspect.
    traces
        Parsed release-admission trace records to inspect.
    identifiers
        Identifiers that must not occur as substrings in string values.
    subject
        Noun phrase used in metric-label and trace-field failure messages.

    Notes
    -----
    Keep record traversal and the metric-label shape check shared across the
    release-admission runtime tests.

    Examples
    --------
    Check both telemetry channels in one call::

        assert_identifiers_excluded_from_records(
            [{"labels": {"operation": "resolve_tag_commit"}}],
            [{"event": "operation_complete"}],
            {"run-7"},
            "generated identifiers",
        )
    """
    for record in metrics:
        labels = record["labels"]
        assert isinstance(labels, dict), "every emitted metric must retain labels"
        assert_identifiers_excluded_from_values(
            labels.values(),
            identifiers,
            f"{subject} must never become metric label values",
        )
    for trace in traces:
        assert_identifiers_excluded_from_values(
            trace.values(),
            identifiers,
            f"{subject} must never become trace field values",
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
