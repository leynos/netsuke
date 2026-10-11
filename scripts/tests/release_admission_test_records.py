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

#: The operation counter's metric name, as the gate renders it.
OPERATION_METRIC = "netsuke_release_admission_operation_total"
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


def describe_run_shape(metrics: list[dict[str, object]]) -> RunShape:
    """Read the operations and outcome back out of a metric artefact.

    The derivation is mechanical -- counter records carry ``outcome`` and
    ``error_category`` labels -- so comparing it against an expected shape does
    not restate production policy, and every duration record is ignored
    deliberately. A clock reading outside the accepted number shape drops that
    duration without touching the counter, so asserting a fixed count of eleven
    records would be a claim about the host clock rather than about the gate.

    Returns
    -------
    RunShape
        The operations in emission order, then the last one's outcome.
    """
    operations, labels = _operations_and_labels(metrics)
    return RunShape(
        operations=operations,
        outcome=str(labels["outcome"]),
        error_category=str(labels["error_category"]),
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
    Call this **before** :func:`assert_identifiers_excluded_from_records` in any
    property. Both that helper and the shared validators accept an empty record
    list, so an exclusion assertion made on its own is satisfied vacuously by a
    gate that wrote nothing at all. Asserting the shape first establishes that
    the records being searched exist.

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


def assert_identifiers_excluded_from_diagnostics(
    stderr: str, identifiers: set[str], subject: str
) -> None:
    r"""Assert identifiers are absent from everything the gate wrote to stderr.

    Every diagnostic the gate prints is a fixed sentence, so an identifier
    reaching standard error is the same cardinality hazard as one reaching a
    metric label: the runner log is retained with the release. The command's own
    stderr is inherited, so an adapter's message reaches this stream too, which
    is exactly why the check belongs here rather than beside the records.

    Examples
    --------
    Check a diagnostic stream that carries no identifier::

        assert_identifiers_excluded_from_diagnostics(
            "release-admission trace sink failed\n",
            {"a" * 40},
            "generated identifiers",
        )
    """
    assert_identifiers_excluded_from_values(
        stderr.splitlines(),
        identifiers,
        f"{subject} must never become a diagnostic on standard error",
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
