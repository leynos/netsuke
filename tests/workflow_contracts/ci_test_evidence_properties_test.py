"""Check generated state and allow-list invariants in the evidence parsers."""

import json
import string

import pytest
from ci_test_evidence_logs import parse_log
from ci_test_evidence_nextest import parse_nextest_records
from ci_test_evidence_telemetry import NUMERIC_FIELDS, apply_inventory
from hypothesis import given, settings
from hypothesis import strategies as st

SAFE_TEST_NAME = st.text(alphabet=string.ascii_lowercase, min_size=1, max_size=8)
KEY_SUFFIX = st.text(alphabet="0123456789abcdef", min_size=1, max_size=12)
INVENTORY_MEASUREMENTS = tuple(
    (section, field)
    for section, fields in NUMERIC_FIELDS.items()
    for field in sorted(fields)
)
INVALID_MEASUREMENT = st.one_of(
    st.booleans(),
    st.integers(max_value=-1),
    st.integers(min_value=2**63),
    st.floats(max_value=-0.1, allow_nan=False, allow_infinity=False),
    st.just(float("nan")),
    st.just(float("inf")),
    st.text(max_size=8),
    st.none(),
)


def _nextest_run(names: list[str]) -> list[tuple[None, str]]:
    """Build one completed run with unique generated test identities."""
    start = f"Starting {len(names)} tests across 1 binary"
    results = [(None, f"PASS [ 0.001s] fixture::unit {name}") for name in names]
    summary = f"Summary [ 0.001s] {len(names)} tests run: {len(names)} passed"
    return [(None, start), *results, (None, summary)]


@given(
    previous=st.lists(SAFE_TEST_NAME, unique=True, max_size=5),
    current=st.lists(SAFE_TEST_NAME, unique=True, max_size=5),
)
def test_nextest_start_scopes_identity_inventory_to_latest_run(
    previous: list[str], current: list[str]
) -> None:
    """Discard every prior identity when a later Nextest run starts."""
    evidence = parse_nextest_records([*_nextest_run(previous), *_nextest_run(current)])

    assert [test["identity"] for test in evidence["tests"]] == [
        f"fixture::unit {name}" for name in current
    ], "The latest run must own the final test identity inventory"
    assert evidence["executed_count"] == len(current), (
        "The latest summary must own the executed count"
    )
    assert evidence["executed_identity_inventory_complete"] is True, (
        "Generated complete runs must reconcile with their identity inventory"
    )


@settings(max_examples=80, derandomize=True, deadline=None)
@given(
    late_count=st.integers(min_value=2, max_value=9),
    late_seconds=st.integers(min_value=1, max_value=9),
)
def test_late_nextest_summary_cannot_replace_closed_run(
    late_count: int, late_seconds: int
) -> None:
    """Keep the first summary's timing, outcomes, and identities authoritative."""
    late_summary = (
        f"Summary [{late_seconds}.000s] {late_count} tests run: {late_count} passed"
    )
    evidence = parse_nextest_records([
        (None, "Starting 1 test across 1 binary"),
        ("2026-10-23T00:00:01Z", "PASS [ 0.100s] fixture::unit original"),
        (None, "Summary [ 0.100s] 1 test run: 1 passed"),
        (None, late_summary),
    ])

    assert evidence["seconds"] == pytest.approx(0.1), (
        "A later summary must not replace the completed run duration"
    )
    assert evidence["selected_count"] == 1, (
        "The original start marker must retain its selected count"
    )
    assert evidence["executed_count"] == 1, (
        "A later summary must not replace the completed run count"
    )
    assert evidence["outcomes"] == {"passed": 1}, (
        "A later summary must not replace the completed run outcomes"
    )
    assert [test["identity"] for test in evidence["tests"]] == [
        "fixture::unit original"
    ], "A later summary must not alter the completed run identity inventory"


@settings(max_examples=80, derandomize=True, deadline=None)
@given(
    attempts=st.lists(
        st.tuples(
            st.sampled_from(("PASS", "FAIL", "TIMEOUT")),
            st.integers(min_value=1, max_value=999),
        ),
        min_size=1,
        max_size=6,
    ),
    late_attempts=st.lists(
        st.tuples(
            st.sampled_from(("PASS", "FAIL", "TIMEOUT")),
            st.integers(min_value=1, max_value=999),
        ),
        max_size=5,
    ),
)
def test_nextest_retries_and_post_summary_lines_keep_final_attempt(
    attempts: list[tuple[str, int]], late_attempts: list[tuple[str, int]]
) -> None:
    """Keep retry identity and final outcome stable after the summary closes."""
    identity = "fixture::unit retry_case"
    result_lines = [
        (
            f"2026-10-23T00:00:{index:02d}Z",
            f"TRY {index} {outcome} [ 0.{milliseconds:03d}s] {identity}",
        )
        for index, (outcome, milliseconds) in enumerate(attempts, start=1)
    ]
    final_outcome, final_milliseconds = attempts[-1]
    summary_outcome = {
        "PASS": "1 passed",
        "FAIL": "1 failed",
        "TIMEOUT": "1 timed out",
    }[final_outcome]
    summary = (
        "2026-10-23T00:01:00Z",
        f"Summary [ 0.100s] 1 test run: {summary_outcome}",
    )
    late_lines = [
        (
            f"2026-10-23T00:02:{index:02d}Z",
            (
                f"TRY {len(attempts) + index} {outcome} "
                f"[ 0.{milliseconds:03d}s] {identity}"
            ),
        )
        for index, (outcome, milliseconds) in enumerate(late_attempts, start=1)
    ]

    evidence = parse_nextest_records([
        (None, "Starting 1 test across 1 binary"),
        *result_lines,
        summary,
        *late_lines,
    ])

    assert len(evidence["tests"]) == 1, (
        "All generated attempts must resolve to one retained identity"
    )
    final_test = evidence["tests"][0]
    assert final_test["identity"] == identity, (
        "The retry history must keep its original test identity"
    )
    assert final_test["outcome"] == final_outcome.lower(), (
        "Only the final pre-summary retry may define the retained outcome"
    )
    assert final_test["seconds"] == pytest.approx(final_milliseconds / 1000), (
        "The retained duration must belong to the final pre-summary retry"
    )
    assert final_test["ended_at"] == result_lines[-1][0], (
        "Late retry-looking output must not replace the final timestamp"
    )
    assert evidence["retries"] == len(attempts) - 1, (
        "Only observed pre-summary attempts after the first count as retries"
    )
    assert evidence["executed_count"] == 1, (
        "Late retry-looking output must not alter the completed summary"
    )


@given(
    batches=st.lists(
        st.tuples(
            st.integers(min_value=0, max_value=9),
            st.integers(min_value=0, max_value=9),
            st.integers(min_value=0, max_value=9),
        ),
        min_size=1,
        max_size=5,
    )
)
def test_doctest_totals_sum_only_completed_target_batches(
    batches: list[tuple[int, int, int]],
) -> None:
    """Keep generated doctest counts target-scoped across Cargo boundaries."""
    lines = []
    for index, counts in enumerate(batches):
        passed, failed, ignored = counts
        result = "FAILED" if failed else "ok"
        total = sum(counts)
        lines.extend([
            f"Doc-tests crate_{index}",
            f"running {total} tests",
            (
                f"test result: {result}. {passed} passed; {failed} failed; "
                f"{ignored} ignored; 0 measured; 0 filtered out"
            ),
            "$ cargo test --doc --workspace",
        ])

    evidence = parse_log("\n".join(lines))

    expected_counts = {
        "passed": sum(counts[0] for counts in batches),
        "failed": sum(counts[1] for counts in batches),
        "ignored": sum(counts[2] for counts in batches),
    }
    assert evidence["doctests"]["counts"] == expected_counts, (
        "Only completed target batches may contribute to aggregate counts"
    )
    assert [section["target"] for section in evidence["doctests"]["sections"]] == [
        f"crate_{index}" for index in range(len(batches))
    ], "Every generated doctest result must retain its own target"


@given(requested=KEY_SUFFIX, restored=st.one_of(st.none(), KEY_SUFFIX))
def test_cache_classification_matches_generated_restore_evidence(
    requested: str, restored: str | None
) -> None:
    """Classify exact, fallback, and cold cache states from observed keys."""
    requested_key = f"netsuke-cargo-v1-{requested}"
    lines = []
    if restored is None:
        lines.append(
            f"Cache not found for input keys: {requested_key}, netsuke-cargo-v1-"
        )
        expected_state = "miss"
    else:
        restored_key = f"netsuke-cargo-v1-{restored}"
        lines.append(f"Cache restored from key: {restored_key}")
        expected_state = "exact" if requested == restored else "fallback"
    lines.append(f"- registry key: `{requested_key}` hit: `false`")

    evidence = parse_log("\n".join(lines))

    assert evidence["caches"]["registry"]["state"] == expected_state, (
        "Cache state must follow the actual requested and restored keys"
    )


@given(
    measurement=st.sampled_from(INVENTORY_MEASUREMENTS),
    value=st.integers(min_value=0, max_value=2**32),
)
def test_inventory_copies_only_generated_allowlisted_measurements(
    measurement: tuple[str, str], value: int
) -> None:
    """Copy one valid measure while excluding generated private metadata."""
    section, field = measurement
    evidence = parse_log("")
    inventory = {
        section: {field: value, "private_label": "must-not-escape"},
        "raw_error": "private-error",
    }

    apply_inventory(evidence, inventory)

    evidence_sections = {
        "profiles": evidence["profiles"],
        "resources": evidence["resources"],
        "nested_cargo": evidence["nested_cargo"],
    }
    for evidence_section, fields in NUMERIC_FIELDS.items():
        for evidence_field in fields:
            expected = (
                value if (evidence_section, evidence_field) == measurement else None
            )
            assert evidence_sections[evidence_section][evidence_field] == expected, (
                "Only the selected allow-listed measure may be copied"
            )
    serialized = json.dumps(evidence)
    assert "private_label" not in serialized, (
        "Arbitrary inventory labels must never enter published evidence"
    )
    assert "private-error" not in serialized, (
        "Raw inventory errors must never enter published evidence"
    )


def test_inventory_accepts_fractional_seconds_only() -> None:
    """Retain fractional durations while rejecting fractional counts and sizes."""
    evidence = parse_log("")
    apply_inventory(
        evidence,
        {
            "profiles": {"export_seconds": 0.25, "raw_count": 1.5},
            "resources": {"cpu_seconds": 1.5, "peak_rss_mib": 2.5},
        },
    )

    assert evidence["profiles"]["export_seconds"] == pytest.approx(0.25), (
        "Fractional seconds are valid duration measurements"
    )
    assert evidence["resources"]["cpu_seconds"] == pytest.approx(1.5), (
        "Fractional CPU seconds are valid duration measurements"
    )
    assert evidence["profiles"]["raw_count"] is None, (
        "Fractional counts must remain unknown"
    )
    assert evidence["resources"]["peak_rss_mib"] is None, (
        "Fractional memory sizes must remain unknown"
    )


@settings(max_examples=80, derandomize=True, deadline=None)
@given(
    measurement=st.sampled_from(INVENTORY_MEASUREMENTS),
    value=INVALID_MEASUREMENT,
)
def test_generated_invalid_inventory_values_remain_unknown(
    measurement: tuple[str, str], value: object
) -> None:
    """Leave malformed, negative, or out-of-range measures unknown."""
    section, field = measurement
    evidence = parse_log("")

    apply_inventory(evidence, {section: {field: value}})

    evidence_sections = {
        "profiles": evidence["profiles"],
        "resources": evidence["resources"],
        "nested_cargo": evidence["nested_cargo"],
    }
    assert evidence_sections[section][field] is None, (
        "Invalid generated telemetry must not become a published measurement"
    )
