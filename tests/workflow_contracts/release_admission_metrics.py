"""Validate the fixed JSON-lines contract for release-admission metrics."""

import json

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
    """Parse non-empty JSON-lines metrics into mapping records."""
    records: list[dict[str, object]] = []
    for line in lines:
        if not line.strip():
            continue
        value = json.loads(line)
        assert isinstance(value, dict), "each metric record must be a JSON object"
        records.append(value)
    return records


def exact_labels(record: dict[str, object]) -> None:
    """Reject a metric record whose labels differ from the fixed contract."""
    name = record.get("name")
    assert isinstance(name, str), f"release-admission metric must be a string: {name!r}"
    assert name in METRIC_LABELS, f"unknown release-admission metric: {name!r}"
    labels = record.get("labels")
    assert isinstance(labels, dict), f"{name} labels must be an object"

    expected = METRIC_LABELS[name]
    expected_names = [label for label, _ in expected]
    assert list(labels) == expected_names, (
        f"{name} labels must be exactly {expected_names!r}, got {list(labels)!r}"
    )
    for label, allowed_values in expected:
        value = labels[label]
        assert isinstance(value, str), f"{name} label {label!r} must be a string"
        assert value in allowed_values, (
            f"{name} label {label!r} must be one of {sorted(allowed_values)!r}"
        )


def is_non_negative_metric_value(value: object) -> bool:
    """Return whether ``value`` is an ordinary non-negative JSON number."""
    match value:
        case bool():
            return False
        case int() | float():
            return value >= 0
        case _:
            return False


def validate_metrics(records: list[dict[str, object]]) -> None:
    """Validate all metric records, including their numeric values."""
    for record in records:
        exact_labels(record)
        value = record.get("value")
        assert is_non_negative_metric_value(value), (
            "metric values must be non-negative JSON numbers"
        )
