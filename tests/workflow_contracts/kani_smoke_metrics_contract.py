"""Validate the bounded JSONL contract emitted by the Kani finalizer."""

import json
import math
import sys
import typing as typ

from workflow_loading import REPO_ROOT

sys.path.insert(0, str(REPO_ROOT / "scripts"))

import kani_proof_scope as scope_mod
import kani_smoke_metrics as metrics_mod

EVENTS = frozenset({"pull_request", "push", "schedule", "workflow_dispatch", "other"})
DECISIONS = frozenset({category.value for category in scope_mod.DecisionCategory}) | {
    "invalid_scope",
    "unknown",
}
OUTCOMES = frozenset({
    "cancelled",
    "scope_failure",
    "skipped",
    "invalid_decision",
    "proof_failure",
    "proof_not_run",
    "mutation_failure",
    "job_failure",
    "success",
    "unknown",
})
CACHE_RESULTS = frozenset({"hit", "miss", "not_attempted", "unknown"})
METRIC_LABELS: dict[str, tuple[tuple[str, frozenset[str]], ...]] = {
    "netsuke_kani_smoke_run_total": (
        ("event", EVENTS),
        ("decision", DECISIONS),
        ("outcome", OUTCOMES),
    ),
    "netsuke_kani_smoke_cache_total": (("result", CACHE_RESULTS),),
    "netsuke_kani_smoke_proof_duration_seconds": (
        ("event", EVENTS),
        ("outcome", OUTCOMES),
    ),
}


def validate_helper_vocabularies() -> None:
    """Keep the producer classifications equal to this independent contract."""
    expected_decisions = {category.value for category in scope_mod.DecisionCategory} | {
        "invalid_scope"
    }
    assert metrics_mod.EVENTS | {"other"} == EVENTS, (
        "the helper event vocabulary must match the contract"
    )
    assert expected_decisions == metrics_mod.DECISIONS, (
        "helper decisions must match DecisionCategory plus invalid_scope"
    )
    assert metrics_mod.OUTCOMES == OUTCOMES, (
        "helper outcomes must match the fixed workflow contract"
    )
    assert metrics_mod.CACHE_RESULTS == CACHE_RESULTS, (
        "helper cache results must match the fixed workflow contract"
    )


def parse_metrics(lines: list[str]) -> list[dict[str, object]]:
    """Decode JSONL and reject non-standard non-finite numbers."""
    return [
        json.loads(line, parse_constant=_reject_non_finite)
        for line in lines
        if line.strip()
    ]


def _reject_non_finite(value: str) -> typ.NoReturn:
    """Reject a JSON constant that is not a finite JSON number."""
    message = f"non-finite JSON metric value: {value}"
    raise ValueError(message)


def validate_metrics(records: list[dict[str, object]]) -> None:
    """Require exact record counts, ordered labels and bounded values."""
    validate_helper_vocabularies()
    names = [record.get("name") for record in records]
    expected = [
        "netsuke_kani_smoke_run_total",
        "netsuke_kani_smoke_cache_total",
    ]
    if len(records) == 3:
        expected.append("netsuke_kani_smoke_proof_duration_seconds")
    assert names == expected, (
        f"metric record sequence must be {expected!r}, got {names!r}"
    )
    for record in records:
        _validate_record(record)


def _validate_record(record: dict[str, object]) -> None:
    """Check a single metric against its exact label and value contract."""
    name = record.get("name")
    assert isinstance(name, str), f"metric name must be a string: {name!r}"
    assert name in METRIC_LABELS, f"unknown metric name: {name!r}"
    assert tuple(record) == ("name", "labels", "value"), (
        f"{name} must contain only name, labels and value"
    )
    labels = record.get("labels")
    assert isinstance(labels, dict), f"{name} labels must be an object"
    expected_labels = METRIC_LABELS[name]
    assert tuple(labels) == tuple(label for label, _ in expected_labels), (
        f"{name} labels must use the fixed ordered keys"
    )
    for label, allowed in expected_labels:
        value = labels[label]
        assert isinstance(value, str), f"{name} {label} must be a string"
        assert value in allowed, f"{name} {label} must use its bounded vocabulary"
    value = record.get("value")
    assert isinstance(value, int | float), f"{name} must have a numeric observation"
    assert not isinstance(value, bool), f"{name} must not use a Boolean observation"
    assert math.isfinite(value), f"{name} value must be finite"
    assert value >= 0, f"{name} value must be non-negative"
    if name.endswith("_total"):
        assert value == 1, f"{name} counter value must be exactly one"
