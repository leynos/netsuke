"""Specify native Nextest start markers and per-run outcome inventories."""

import pytest
from ci_test_evidence_nextest import nextest_phase_marker, parse_nextest_records


@pytest.mark.parametrize(
    ("start", "selected", "skipped", "discovered"),
    [
        ("Starting 5 tests across 2 binaries", 5, 0, 5),
        ("Starting 5 tests across 2 binaries (3 tests skipped)", 5, 3, 8),
        (
            (
                "Starting 5 tests across 2 binaries "
                "(3 tests and 1 binary skipped via profile.default.default-filter)"
            ),
            5,
            3,
            8,
        ),
        ("Starting 5 tests across 2 binaries (1 binary skipped)", 5, None, None),
    ],
)
def test_nextest_start_lines_preserve_only_observed_counts(
    start: str,
    selected: int,
    skipped: int | None,
    discovered: int | None,
) -> None:
    """Parse documented start-line forms without inventing skipped tests."""
    evidence = parse_nextest_records([(None, start)])

    assert evidence["selected_count"] == selected, (
        "The selected test count must survive each start-line form"
    )
    assert evidence["skipped_count"] == skipped, (
        "Skipped test counts must remain unknown when only binaries are reported"
    )
    assert evidence["discovered_count"] == discovered, (
        "Discovered tests must remain unknown without a skipped-test count"
    )
    assert evidence["identity_inventory_complete"] is False, (
        "A start marker alone cannot establish a complete identity inventory"
    )
    assert nextest_phase_marker(start) == "nextest_started", (
        "All documented start-line forms must mark the start phase"
    )


def test_new_nextest_start_replaces_the_previous_run_inventory() -> None:
    """Keep identities and summary counts scoped to the most recent run."""
    evidence = parse_nextest_records([
        (None, "Starting 1 test across 1 binary"),
        (None, "PASS [ 0.100s] fixture::unit first_run"),
        (None, "Summary [ 0.100s] 1 test run: 1 passed"),
        (
            None,
            (
                "Starting 1 test across 1 binary "
                "(2 tests and 1 binary skipped via profile.default.default-filter)"
            ),
        ),
        (None, "PASS [ 0.200s] fixture::unit second_run"),
        (None, "Summary [ 0.200s] 1 test run: 1 passed"),
    ])

    assert evidence["executed_count"] == 1, (
        "The latest Nextest summary must supply the executed count"
    )
    assert [test["identity"] for test in evidence["tests"]] == [
        "fixture::unit second_run"
    ], "Starting a later run must discard earlier run identities"
    assert evidence["executed_identity_inventory_complete"] is True, (
        "The latest run inventory must reconcile with its summary"
    )
    assert evidence["skipped_count"] == 2, (
        "Skipped test count from the latest run must remain associated"
    )


def test_skipped_test_identity_is_retained_but_not_counted_as_executed() -> None:
    """Reconcile blank-duration skip lines with Nextest's zero-run summary."""
    evidence = parse_nextest_records([
        (None, "Starting 0 tests across 1 binary (1 test skipped)"),
        (None, "SKIP [         ] fixture::unit ignored"),
        (None, "Summary [ 0.000s] 0 tests run: 0 passed, 1 skipped"),
    ])

    expected_test = {
        "identity": "fixture::unit ignored",
        "outcome": "skip",
        "seconds": None,
        "ended_at": None,
    }
    assert evidence["tests"] == [expected_test], (
        "Skipped tests must remain visible even without a duration"
    )
    assert evidence["executed_count"] == 0, (
        "Nextest excludes skipped tests from its executed count"
    )
    assert evidence["executed_identity_inventory_complete"] is True, (
        "The executed identity set must reconcile after excluding skips"
    )
    assert evidence["identity_inventory_complete"] is False, (
        "A skipped test prevents claiming a complete selected identity set"
    )


def test_partial_nextest_summary_preserves_selected_and_executed_counts() -> None:
    """Keep fail-fast runs distinct from complete selected identity inventories."""
    evidence = parse_nextest_records([
        (None, "Starting 2 tests across 1 binary"),
        (None, "PASS [ 0.100s] fixture::unit completed"),
        (None, "Summary [ 0.120s] 1/2 tests run: 1 passed"),
    ])

    assert evidence["seconds"] == pytest.approx(0.12), (
        "A partial summary must retain its native elapsed time"
    )
    assert evidence["selected_count"] == 2, (
        "The start marker must retain the full selected test count"
    )
    assert evidence["executed_count"] == 1, (
        "The summary numerator must supply the executed test count"
    )
    assert evidence["outcomes"] == {"passed": 1}, (
        "Outcome totals must survive an optional summary denominator"
    )
    assert evidence["executed_identity_inventory_complete"] is True, (
        "All observed executed identities must still reconcile with the summary"
    )
    assert evidence["identity_inventory_complete"] is False, (
        "Unstarted selected tests must prevent a complete inventory claim"
    )
    assert nextest_phase_marker("Summary [ 0.120s] 1/2 tests run: 1 passed") == (
        "nextest_completed"
    ), "Partial summaries must still close the Nextest phase"


def test_late_nextest_summary_cannot_replace_a_closed_run() -> None:
    """Keep summary counts and elapsed time fixed until the next run starts."""
    evidence = parse_nextest_records([
        (None, "Starting 1 test across 1 binary"),
        (None, "PASS [ 0.100s] fixture::unit works"),
        (None, "Summary [ 0.100s] 1 test run: 1 passed"),
        (None, "Summary [ 9.000s] 2 tests run: 2 passed"),
    ])

    assert evidence["seconds"] == pytest.approx(0.1), (
        "A summary after closure must not replace the completed run duration"
    )
    assert evidence["executed_count"] == 1, (
        "A summary after closure must not replace the completed run count"
    )
    assert evidence["outcomes"] == {"passed": 1}, (
        "A summary after closure must not replace the completed run outcomes"
    )
    assert [test["identity"] for test in evidence["tests"]] == [
        "fixture::unit works"
    ], "A completed run must retain only its own test identities"


@pytest.mark.parametrize("status", ["ABORT", "SIGABRT", "SIGTERM"])
def test_crashed_test_statuses_remain_in_the_identity_inventory(status: str) -> None:
    """Retain identities for aborted and signal-terminated test processes."""
    evidence = parse_nextest_records([
        (None, "Starting 1 test across 1 binary"),
        (None, f"{status} [ 0.100s] fixture::unit crashed"),
        (None, "Summary [ 0.100s] 1 test run: 1 failed"),
    ])

    expected_test = {
        "identity": "fixture::unit crashed",
        "outcome": status.lower(),
        "seconds": 0.1,
        "ended_at": None,
    }
    assert evidence["tests"] == [expected_test], (
        "Aborted executions must remain visible as executed identities"
    )
    assert evidence["executed_identity_inventory_complete"] is True, (
        "A crash outcome must still reconcile with the test summary"
    )
