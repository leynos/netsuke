"""Property checks for bounded Kani metric labels and observations."""

import sys

from hypothesis import given, settings
from hypothesis import strategies as st
from workflow_loading import REPO_ROOT

sys.path.insert(0, str(REPO_ROOT / "scripts"))

import kani_proof_scope as scope_mod
import kani_smoke_metrics as metrics
from kani_smoke_metrics_contract import DECISIONS, validate_metrics

PROPERTY = settings(max_examples=200, deadline=None)
TEXT = st.text(max_size=200)


@PROPERTY
@given(event=TEXT, decision=TEXT, status=TEXT, proof_status=TEXT)
def test_arbitrary_inputs_always_emit_bounded_labels(
    event: str, decision: str, status: str, proof_status: str
) -> None:
    """Collapse arbitrary workflow text before it reaches metric labels."""
    inputs = metrics.WorkflowInputs(
        event,
        status,
        status,
        status,
        status,
        status,
        proof_status,
        status,
    )
    records = metrics.metrics_records(decision, inputs, 10, 20)
    validate_metrics(records)
    labels = records[0]["labels"]
    assert isinstance(labels, dict), "run observations must retain a label object"
    raw_values = {event, decision, status, proof_status} - {
        "pull_request",
        "push",
        "schedule",
        "workflow_dispatch",
        "other",
        *DECISIONS,
        *metrics.OUTCOMES,
    }
    assert not raw_values.intersection(labels.values()), (
        "unrecognised source strings must not become metric label values"
    )


def test_decision_vocabulary_matches_scope_categories() -> None:
    """Keep the metrics vocabulary aligned with the scope helper's public enum."""
    expected = {category.value for category in scope_mod.DecisionCategory} | {
        "invalid_scope",
        "unknown",
    }
    assert expected == DECISIONS, (
        "decision metrics must cover the exact fixed vocabulary"
    )


@PROPERTY
@given(start=st.integers(min_value=0), end=st.integers(min_value=0))
def test_duration_never_becomes_negative_or_non_finite(start: int, end: int) -> None:
    """Clamp reversed monotonic marks and retain a finite duration record."""
    records = metrics.metrics_records(
        "full_run",
        metrics.WorkflowInputs(
            "push",
            "success",
            "success",
            "true",
            "success",
            "false",
            "success",
            "success",
        ),
        start,
        end,
    )
    duration = records[2]["value"]
    assert isinstance(duration, int | float), "duration must be numeric"
    assert duration >= 0, "every duration must be non-negative"
