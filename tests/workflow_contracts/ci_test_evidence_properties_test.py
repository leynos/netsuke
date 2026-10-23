"""Check generated state and allow-list invariants in the evidence parsers."""

import json
import string

from ci_test_evidence_logs import parse_log
from ci_test_evidence_nextest import parse_nextest_records
from ci_test_evidence_telemetry import NUMERIC_FIELDS, apply_inventory
from hypothesis import given
from hypothesis import strategies as st

SAFE_TEST_NAME = st.text(alphabet=string.ascii_lowercase, min_size=1, max_size=8)
KEY_SUFFIX = st.text(alphabet="0123456789abcdef", min_size=1, max_size=12)
INVENTORY_MEASUREMENTS = tuple(
    (section, field)
    for section, fields in NUMERIC_FIELDS.items()
    for field in sorted(fields)
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
