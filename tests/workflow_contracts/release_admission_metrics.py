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
    records: list[dict[str, object]] = []
    for line in lines:
        if not line.strip():
            continue
        try:
            value = json.loads(line, parse_constant=_reject_non_finite_json_number)
        except (json.JSONDecodeError, ValueError) as error:
            message = "release-admission metric records must contain finite JSON"
            raise ValueError(message) from error
        if not isinstance(value, dict):
            message = "each metric record must be a JSON object"
            raise TypeError(message)
        records.append(value)
    return records


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
        If the metric name, label order, label names, or label values fall
        outside the fixed release-admission vocabulary.
    TypeError
        If the metric name, labels object, or a label value has the wrong type.

    Notes
    -----
    Exact ordering prevents silently accepting an extra, unbounded label.
    """
    name = record.get("name")
    if not isinstance(name, str):
        message = f"release-admission metric must be a string: {name!r}"
        raise TypeError(message)
    if name not in METRIC_LABELS:
        message = f"unknown release-admission metric: {name!r}"
        raise AssertionError(message)
    labels = record.get("labels")
    if not isinstance(labels, dict):
        message = f"{name} labels must be an object"
        raise TypeError(message)

    expected = METRIC_LABELS[name]
    expected_names = [label for label, _ in expected]
    if list(labels) != expected_names:
        message = (
            f"{name} labels must be exactly {expected_names!r}, got {list(labels)!r}"
        )
        raise AssertionError(message)
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
    match value:
        case bool():
            return False
        case int():
            return value >= 0
        case float():
            return math.isfinite(value) and value >= 0
        case _:
            return False


def validate_metrics(records: list[dict[str, object]]) -> None:
    """Validate every record in a release-admission metrics export.

    Parameters
    ----------
    records
        Decoded metric records from :func:`parse_metrics`.

    Raises
    ------
    AssertionError
        If any record violates the fixed labels contract or has an invalid
        metric value.

    Notes
    -----
    Each record must remain within the bounded metric vocabulary before an
    operator or test consumes it.
    """
    for record in records:
        exact_labels(record)
        value = record.get("value")
        if not is_non_negative_metric_value(value):
            message = "metric values must be non-negative JSON numbers"
            raise AssertionError(message)
