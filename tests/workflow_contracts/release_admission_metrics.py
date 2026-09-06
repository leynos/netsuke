"""Validate the fixed JSON-lines contract for release-admission metrics."""

import json
import math

type MetricLabels = tuple[tuple[str, frozenset[str]], ...]

CANARIES = frozenset({"history_scan", "release_candidate", "none"})
OPERATIONS = frozenset({
    "resolve_tag_commit",
    "fetch_candidate_revision",
    "fetch_workflow_run",
    "check_scan_freshness",
    "verify_evidence",
})
OUTCOMES = frozenset({"success", "failure", "unknown"})
ERROR_CATEGORIES = frozenset({
    "none",
    "api_error",
    "fetch_error",
    "stale_evidence",
    "missing_evidence",
    "mismatch",
    "timeout",
    "unknown",
})

METRIC_LABELS: dict[str, MetricLabels] = {
    "netsuke_release_admission_gate_total": (
        ("outcome", OUTCOMES),
        ("error_category", ERROR_CATEGORIES),
    ),
    "netsuke_release_admission_operation_total": (
        ("canary", CANARIES),
        ("operation", OPERATIONS),
        ("outcome", OUTCOMES),
        ("error_category", ERROR_CATEGORIES),
    ),
    "netsuke_release_admission_operation_duration_seconds": (
        ("operation", OPERATIONS),
    ),
}

TRACE_EVENTS = frozenset({
    "operation_complete",
    "gate_complete",
    "workflow_output_delivery",
    "trace_delivery",
})
TRACE_FIELDS = (
    "event",
    "operation",
    "outcome",
    "error_category",
    "duration_seconds",
)


def parse_metrics(lines: list[str]) -> list[dict[str, object]]:
    """Parse non-empty JSON Lines metric records.

    Parameters
    ----------
    lines
        JSON Lines records emitted by the release-admission gate.

    Returns
    -------
    list[dict[str, object]]
        Decoded metric mappings in input order.

    Raises
    ------
    TypeError
        If a decoded record is not a JSON object.
    ValueError
        If a record is malformed JSON or contains a non-finite JSON number.

    Notes
    -----
    The parser rejects `NaN`, `Infinity`, and `-Infinity` so the validator
    accepts only portable JSON metric values.
    """
    return _parse_json_lines(lines, "metric")


def _reject_non_finite_json_number(value: str) -> object:
    """Reject JSON constants that cannot represent finite metric values."""
    message = f"non-finite JSON metric value: {value}"
    raise ValueError(message)


def exact_labels(record: dict[str, object]) -> None:
    """Validate that a metric record has exactly the contracted labels.

    Parameters
    ----------
    record
        One decoded JSON metric record.

    Raises
    ------
    AssertionError
        If the record's label names or order fall outside the fixed contract.

    Notes
    -----
    Exact ordering prevents silently accepting an extra, unbounded label.
    """
    name, expected = _metric_definition(record)
    labels = _metric_labels(record, name)
    expected_names = [label for label, _ in expected]
    if list(labels) != expected_names:
        message = (
            f"{name} labels must be exactly {expected_names!r}, got {list(labels)!r}"
        )
        raise AssertionError(message)
    _validate_label_values(name, labels, expected)


def _metric_definition(record: dict[str, object]) -> tuple[str, MetricLabels]:
    """Return the fixed label definition for the metric record's name."""
    name = record.get("name")
    if not isinstance(name, str):
        message = f"release-admission metric must be a string: {name!r}"
        raise TypeError(message)
    if name not in METRIC_LABELS:
        message = f"unknown release-admission metric: {name!r}"
        raise AssertionError(message)
    return name, METRIC_LABELS[name]


def _metric_labels(record: dict[str, object], name: str) -> dict[str, object]:
    """Return the labels object after ensuring it is a mapping."""
    labels = record.get("labels")
    if not isinstance(labels, dict):
        message = f"{name} labels must be an object"
        raise TypeError(message)
    return labels


def _validate_label_values(
    name: str,
    labels: dict[str, object],
    expected: MetricLabels,
) -> None:
    """Reject labels whose values fall outside the fixed metric vocabulary."""
    for label, allowed_values in expected:
        value = labels[label]
        if not isinstance(value, str):
            message = f"{name} label {label!r} must be a string"
            raise TypeError(message)
        if value not in allowed_values:
            message = (
                f"{name} label {label!r} must be one of {sorted(allowed_values)!r}"
            )
            raise AssertionError(message)


def is_non_negative_metric_value(value: object) -> bool:
    """Return whether a metric value is finite, numeric, and non-negative.

    Parameters
    ----------
    value
        A decoded JSON value proposed as a metric observation.

    Returns
    -------
    bool
        `True` only for finite integer or floating-point values at least zero.

    Notes
    -----
    Boolean values and non-finite floats are rejected even though Python treats
    booleans as integers and its JSON decoder can represent infinity.
    """
    if not isinstance(value, int | float) or isinstance(value, bool):
        return False
    return math.isfinite(value) and value >= 0


def validate_metrics(records: list[dict[str, object]]) -> None:
    """Validate every record in a release-admission metrics export.

    Parameters
    ----------
    records
        Decoded metric records from :func:`parse_metrics`.

    Notes
    -----
    Each record must remain within the bounded metric vocabulary before an
    operator or test consumes it.
    """
    for record in records:
        exact_labels(record)
        _validate_metric_value(record)


def parse_traces(lines: list[str]) -> list[dict[str, object]]:
    """Parse finite JSON Lines release-admission trace records.

    Parameters
    ----------
    lines
        JSON Lines records emitted by the bounded trace sink.

    Returns
    -------
    list[dict[str, object]]
        Decoded trace mappings in input order.

    """
    return _parse_json_lines(lines, "trace")


def validate_traces(records: list[dict[str, object]]) -> None:
    """Validate the fixed field and value contract for trace records.

    Parameters
    ----------
    records
        Decoded trace records from :func:`parse_traces`.

    Raises
    ------
    AssertionError
        If fields, vocabulary, or duration leave the fixed trace contract.

    Notes
    -----
    The exact schema prevents identifiers or raw diagnostic data from entering
    the trace export.
    """
    for record in records:
        if tuple(record) != TRACE_FIELDS:
            message = f"trace fields must be exactly {TRACE_FIELDS!r}"
            raise AssertionError(message)
        _validate_trace_value(record, "event", TRACE_EVENTS)
        _validate_trace_value(record, "operation", OPERATIONS)
        _validate_trace_value(record, "outcome", OUTCOMES)
        _validate_trace_value(record, "error_category", ERROR_CATEGORIES)
        if not is_non_negative_metric_value(record["duration_seconds"]):
            message = "trace duration_seconds must be a finite non-negative number"
            raise AssertionError(message)


def _validate_metric_value(record: dict[str, object]) -> None:
    """Reject a metric observation that is not a finite non-negative number."""
    value = record.get("value")
    if not is_non_negative_metric_value(value):
        message = "metric values must be non-negative JSON numbers"
        raise AssertionError(message)


def _parse_json_lines(lines: list[str], record_kind: str) -> list[dict[str, object]]:
    """Decode finite JSON Lines mappings for one bounded export kind."""
    records: list[dict[str, object]] = []
    for line in lines:
        if not line.strip():
            continue
        try:
            value = json.loads(line, parse_constant=_reject_non_finite_json_number)
        except (json.JSONDecodeError, ValueError) as error:
            message = (
                f"release-admission {record_kind} records must contain finite JSON"
            )
            raise ValueError(message) from error
        if not isinstance(value, dict):
            message = f"each {record_kind} record must be a JSON object"
            raise TypeError(message)
        records.append(value)
    return records


def _validate_trace_value(
    record: dict[str, object], field: str, allowed_values: frozenset[str]
) -> None:
    """Reject one trace field outside its bounded string vocabulary."""
    value = record[field]
    if not isinstance(value, str):
        message = f"trace field {field!r} must be a string"
        raise TypeError(message)
    if value not in allowed_values:
        message = f"trace field {field!r} falls outside the fixed vocabulary"
        raise AssertionError(message)
