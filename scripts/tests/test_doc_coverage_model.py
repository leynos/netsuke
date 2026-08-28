"""Test documentation-coverage value objects."""

import pytest
from doc_coverage_model import Coverage


def test_aggregation_sums_targets_and_reports_percentage() -> None:
    """Aggregate totals roll per-target counts up and report the share."""
    first = Coverage(10, 8)
    second = Coverage(5, 5)

    combined = first + second

    assert combined.total == 15
    assert combined.with_docs == 13
    assert combined.percentage == pytest.approx(13 / 15 * 100)


def test_empty_run_is_complete_not_a_division_by_zero() -> None:
    """Treat a crate with no doc-able targets as a complete, empty run."""
    assert Coverage(0, 0).percentage == 100.0
