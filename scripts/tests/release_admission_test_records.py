"""Provide fixed-record assertions shared by release-admission runtime tests.

The identifier-exclusion helpers live in
:mod:`release_admission_test_exclusions`, which the support facade re-exports
alongside this module's shape assertions.
"""

import math
import typing as typ

OPERATION_SEQUENCE = (
    "resolve_tag_commit",
    "fetch_candidate_revision",
    "fetch_workflow_run",
    "check_scan_freshness",
    "verify_evidence",
)

#: The operation counter's metric name, as the gate renders it.
OPERATION_METRIC = "netsuke_release_admission_operation_total"
#: The gate counter's metric name. It is the run's final record, because the
#: gate writes it in ``finish`` after the operation chain has stopped.
GATE_METRIC = "netsuke_release_admission_gate_total"
#: The duration metric's name, needed only to identify it when skipping.
DURATION_METRIC = "netsuke_release_admission_operation_duration_seconds"

#: The gate trace, which reports the *gate's* outcome rather than a success.
GATE_TRACE = "gate_complete"
#: The two delivery traces, which are always successful when the gate got this
#: far: the first reports that the workflow outputs were written and the second
#: that the traces were. Both are reached only after the gate trace.
DELIVERY_TRACES = ("workflow_output_delivery", "trace_delivery")


class RunShape(typ.NamedTuple):
    """Name the records a gate run is expected to have emitted.

    The shape separates the operations that ran from the outcome of the last
    one, because "five operations ran" and "the fifth refused the evidence" are
    independent claims and a failure should name which was wrong.
    """

    operations: tuple[str, ...]
    outcome: str
    error_category: str


def _operations_and_labels(
    metrics: list[dict[str, object]],
) -> tuple[tuple[str, ...], dict[str, object]]:
    """Return the operation counters' names and the last one's labels.

    Returns
    -------
    tuple[tuple[str, ...], dict[str, object]]
        Every operation name in emission order, and the labels of the last one.

    Notes
    -----
    An artefact carrying no operation counter at all is the vacuity this
    module's shape assertion exists to refuse, so it is rejected with an
    assertion rather than left to an ``IndexError`` on the last element: a
    record-less run then fails naming that fact instead of reporting a
    subscript fault.
    """
    counters = [
        record
        for record in metrics
        if record["name"] == OPERATION_METRIC and isinstance(record["labels"], dict)
    ]
    assert counters, "the run must have emitted at least one operation counter"
    operations = tuple(
        str(typ.cast("dict[str, object]", counter["labels"])["operation"])
        for counter in counters
    )
    return operations, typ.cast("dict[str, object]", counters[-1]["labels"])


def _gate_labels(metrics: list[dict[str, object]]) -> dict[str, object]:
    """Return the gate counter's labels, which are the run's own outcome.

    Returns
    -------
    dict[str, object]
        The ``outcome`` and ``error_category`` labels of the final record.

    Notes
    -----
    The gate counter is the last record, so its labels are read off the end of
    the artefact rather than searched for; a run whose final record is not the
    gate counter is a shape fault this helper reports rather than silently
    accepting.
    """
    assert metrics, "the run must have emitted at least one metric record"
    assert metrics[-1]["name"] == GATE_METRIC, (
        "the run's final record must be the gate counter"
    )
    labels = metrics[-1]["labels"]
    assert isinstance(labels, dict), "the gate counter must retain labels"
    return labels


def describe_run_shape(metrics: list[dict[str, object]]) -> RunShape:
    """Read the operations and outcome back out of a metric artefact.

    The derivation is mechanical -- counter records carry ``outcome`` and
    ``error_category`` labels -- so comparing it against an expected shape does
    not restate production policy, and every duration record is ignored
    deliberately. A clock reading outside the accepted number shape drops that
    duration without touching the counter, so asserting a fixed count of eleven
    records would be a claim about the host clock rather than about the gate.

    The operation labels and the gate labels are cross-checked here rather than
    in the caller, because they are two independent emissions of one outcome
    and a run that disagreed between them would otherwise satisfy either check
    on its own.

    Returns
    -------
    RunShape
        The operations in emission order, then the last one's outcome.
    """
    operations, last_operation_labels = _operations_and_labels(metrics)
    gate_labels = _gate_labels(metrics)
    final_outcome = str(last_operation_labels["outcome"])
    final_category = str(last_operation_labels["error_category"])
    assert (str(gate_labels["outcome"]), str(gate_labels["error_category"])) == (
        final_outcome,
        final_category,
    ), "the gate counter must repeat the final operation's outcome and category"
    return RunShape(
        operations=operations,
        outcome=final_outcome,
        error_category=final_category,
    )


def assert_complete_records(
    metrics: list[dict[str, object]],
    traces: list[dict[str, object]],
    shape: RunShape,
) -> None:
    """Assert a run emitted exactly the records *shape* names, before exclusion.

    Parameters
    ----------
    metrics
        Decoded metric records from one run.
    traces
        Decoded trace records from one run.
    shape
        The operations, final outcome, and final category the run must show.

    Notes
    -----
    Call this **before**
    :func:`release_admission_test_exclusions.assert_identifiers_excluded_from_records`
    in any property. Both that helper and the shared validators accept an empty record
    list, so an exclusion assertion made on its own is satisfied vacuously by a
    gate that wrote nothing at all. Asserting the shape first establishes that
    the records being searched exist.

    Both telemetry channels are covered: the metric artefact must carry the
    operation counters *and* the gate counter the run finishes with, and the
    trace artefact must carry the full hand-off sequence. A run that wrote
    metrics but no traces, or traces but no metrics, fails here rather than
    passing an exclusion check on one channel alone.

    The trace sequence is asserted exactly rather than by length: every
    predecessor succeeded, the final operation reached *shape.outcome*, the gate
    trace repeated that outcome, and the two delivery traces followed it
    successfully. The gate trace carries the *gate's* result rather than a
    per-operation one -- both are the final operation's here, because the gate
    stops at the first failure -- and the delivery traces succeed because both
    channels remain writable when the records are present at all.
    """
    assert describe_run_shape(metrics) == shape, (
        "the run's metric records must name the operations and outcome asserted"
    )
    final_operation = shape.operations[-1]
    expected_traces = (
        [
            ("operation_complete", operation, "success", "none")
            for operation in shape.operations[:-1]
        ]
        + [
            (
                "operation_complete",
                final_operation,
                shape.outcome,
                shape.error_category,
            ),
            (GATE_TRACE, final_operation, shape.outcome, shape.error_category),
        ]
        + [(event, final_operation, "success", "none") for event in DELIVERY_TRACES]
    )
    assert [
        tuple(
            trace[field]
            for field in ("event", "operation", "outcome", "error_category")
        )
        for trace in traces
    ] == expected_traces, "every bounded trace hand-off must be present exactly once"


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
