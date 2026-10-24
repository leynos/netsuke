"""Specify log parsing at ambiguous progress and coverage boundaries."""

import pytest
from ci_test_evidence_logs import parse_log


@pytest.mark.parametrize("binary", ["netsuke-build", "netsuke-build::bin/netsuke"])
def test_padded_nextest_progress_retains_unit_identity(binary: str) -> None:
    """Native progress padding must not discard the first thousand results."""
    evidence = parse_log(
        f"        PASS [ 0.046s] (   1/3912) {binary} cli::config::works"
    )
    assert evidence["nextest"]["tests"][0]["identity"] == (
        f"{binary} cli::config::works"
    ), "Progress padding and binary paths must not lose test identities"


def test_malformed_native_numbers_remain_missing() -> None:
    """Reject malformed measurements without silently promoting invalid facts."""
    evidence = parse_log(
        "Current coverage: 1.2.3%\n"
        " Summary [ 1..2s] 2 tests run: 2 passed\n"
        '{"stats":null}'
    )
    assert evidence["coverage"]["current_percent"] is None, (
        "Malformed percentages must remain unknown"
    )
    assert evidence["nextest"]["seconds"] is None, (
        "Malformed native timings must remain unknown"
    )
    assert evidence["sccache"]["compile_requests"] is None, (
        "Malformed statistics envelopes must remain unknown"
    )


@pytest.mark.parametrize("boundary", ["all doctests ran in 1.59s", "##[endgroup]"])
def test_merged_doctest_batches_keep_compile_fail_counts(boundary: str) -> None:
    """Retain every native batch while excluding later unrelated harnesses."""
    evidence = parse_log(
        "\n".join([
            "   Doc-tests netsuke",
            "running 114 tests",
            "test result: ok. 88 passed; 0 failed; 26 ignored;",
            "running 2 tests",
            "test result: ok. 2 passed; 0 failed; 0 ignored;",
            boundary,
            "running 999 tests",
            "test result: ok. 999 passed; 0 failed; 0 ignored;",
            "   Doc-tests test_support",
            "running 45 tests",
            "test result: ok. 39 passed; 0 failed; 6 ignored;",
        ])
    )
    assert evidence["doctests"]["counts"] == {
        "passed": 129,
        "failed": 0,
        "ignored": 32,
    }, "Merged and compile-fail batches must both contribute to doctest totals"
    assert [section["target"] for section in evidence["doctests"]["sections"]] == [
        "netsuke",
        "netsuke",
        "test_support",
    ], "Each native result batch must retain its documentation target"


@pytest.mark.parametrize("marker_before_result", [True, False])
def test_interleaved_doctest_marker_keeps_pending_target(
    *, marker_before_result: bool
) -> None:
    """Keep a pending target when an earlier target finishes after its marker."""
    marker = "   Doc-tests test_support"
    result = "test result: ok. 2 passed; 0 failed; 0 ignored;"
    interleaved = [marker, result] if marker_before_result else [result, marker]
    evidence = parse_log(
        "\n".join([
            "   Doc-tests netsuke",
            "running 114 tests",
            "test result: ok. 88 passed; 0 failed; 26 ignored;",
            "running 2 tests",
            *interleaved,
            "all doctests ran in 1.39s; merged doctests compilation took 1.15s",
            "running 45 tests",
            "test result: ok. 39 passed; 0 failed; 6 ignored;",
            "all doctests ran in 0.92s; merged doctests compilation took 0.86s",
            "running 999 tests",
            "test result: ok. 999 passed; 0 failed; 0 ignored;",
        ])
    )
    assert evidence["doctests"]["counts"] == {
        "passed": 129,
        "failed": 0,
        "ignored": 32,
    }, "One target's completion must not discard the pending target's results"
    assert [section["target"] for section in evidence["doctests"]["sections"]] == [
        "netsuke",
        "netsuke",
        "test_support",
    ], "An interleaved marker must not reassign an already-running batch"


def test_missing_archive_size_does_not_reuse_previous_restore() -> None:
    """Keep an omitted size unknown rather than borrowing another archive's."""
    evidence = parse_log(
        "Cache Size: ~1 MB (1234567 B)\n"
        "Cache restored from key: netsuke-cargo-v1-old\n"
        "Cache restored from key: netsuke-tools-v1-old\n"
        "- registry key: `netsuke-cargo-v1-old` hit: `true`\n"
        "- tools key: `netsuke-tools-v1-old` hit: `true`"
    )
    assert evidence["caches"]["registry"]["archive_bytes"] == 1234567, (
        "The observed archive must keep its native byte count"
    )
    assert evidence["caches"]["tools"]["archive_bytes"] is None, (
        "A restored archive without its own size must remain unknown"
    )
