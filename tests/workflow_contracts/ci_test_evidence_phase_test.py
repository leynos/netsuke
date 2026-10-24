"""Check summary-boundary attribution in native CI log phases."""

import pytest
from ci_test_evidence_logs import parse_log


def test_late_nextest_summary_does_not_replace_closed_phase_timestamp() -> None:
    """Keep phase attribution aligned with the immutable Nextest summary."""
    evidence = parse_log(
        "\n".join([
            "2026-10-12T00:00:00Z Starting 1 test across 1 binary",
            "2026-10-12T00:00:01Z PASS [ 0.100s] fixture::unit works",
            "2026-10-12T00:00:02Z Summary [ 0.100s] 1 test run: 1 passed",
            "2026-10-12T00:00:09Z Summary [ 9.000s] 2 tests run: 2 passed",
        ])
    )

    assert evidence["nextest"]["seconds"] == pytest.approx(0.1), (
        "The parsed summary must remain the first one after the start marker"
    )
    phases = evidence["native_phases"]
    assert len(phases) == 2, "A late summary must not add another phase boundary"
    assert phases[0]["phase"] == "nextest_started", (
        "The original start marker must remain"
    )
    assert phases[0]["timestamp"] == "2026-10-12T00:00:00Z", (
        "The original start timestamp must remain"
    )
    assert phases[1]["phase"] == "nextest_completed", (
        "The original completion marker must remain"
    )
    assert phases[1]["timestamp"] == "2026-10-12T00:00:02Z", (
        "A late summary must not move the completed phase boundary"
    )
