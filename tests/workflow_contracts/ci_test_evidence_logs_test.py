"""Specify safe native-log extraction without inferring missing measurements."""

import json

import pytest
from ci_test_evidence_logs import parse_log


def test_native_results_and_cache_fallback_are_not_guessed() -> None:
    """Read observed native records and retain uncertainty in old cache output."""
    evidence = parse_log(
        "\n".join([
            (
                "2026-10-12T00:00:00Z - registry key: "
                "`netsuke-cargo-v1-Linux-x` hit: `false`"
            ),
            (
                "2026-10-12T00:00:01Z - tools key: "
                "`netsuke-tools-v1-Linux-x` hit: `true`"
            ),
            (
                "2026-10-12T00:00:02Z     Starting 2 tests across 1 binaries "
                "(1 tests skipped)"
            ),
            "2026-10-12T00:00:03Z         PASS [ 0.100s] (1/2) netsuke::unit works",
            "2026-10-12T00:00:04Z         FAIL [ 0.200s] (2/2) netsuke::unit fails",
            (
                "2026-10-12T00:00:04Z      Summary [ 2.000s] 2 tests run: "
                "1 passed, 1 failed, 1 skipped"
            ),
            "2026-10-12T00:00:05Z    Doc-tests netsuke",
            (
                "2026-10-12T00:00:06Z test result: ok. 1 passed; 0 failed; 2 ignored; "
                "0 measured; 0 filtered out; finished in 0.01s"
            ),
            "2026-10-12T00:00:07Z Cache hits 10",
            "2026-10-12T00:00:07Z Cache misses 2",
            "2026-10-12T00:00:08Z - memory: 3000 MiB on 4 vCPUs",
        ])
    )
    assert evidence["caches"]["registry"]["state"] == "non_exact_unknown", (
        "An old false hit output cannot distinguish fallback from miss"
    )
    assert evidence["caches"]["registry"]["matched_key"] is None, (
        "Missing matched keys must remain unknown"
    )
    assert evidence["caches"]["tools"]["state"] == "exact", (
        "An explicit exact hit must remain exact"
    )
    assert evidence["nextest"]["seconds"] == 2, (
        "Nextest native wall time must be retained"
    )
    assert evidence["nextest"]["selected_count"] == 2, (
        "Selected count must exclude skipped tests"
    )
    assert evidence["nextest"]["discovered_count"] == 3, (
        "Discovered count must include skipped tests"
    )
    assert evidence["nextest"]["tests"][0]["identity"] == "netsuke::unit works", (
        "Native test identity must include its binary"
    )
    assert evidence["doctests"]["counts"] == {"passed": 1, "failed": 0, "ignored": 2}, (
        "Doctest counters must come from the documentation target"
    )
    assert evidence["sccache"]["hits"] == 10, (
        "Native compiler-cache hits must be retained"
    )
    assert evidence["resources"]["system_used_peak_mib"] == 3000, (
        "System-used memory must retain its original meaning"
    )
    assert evidence["nextest"]["retries"] is None, (
        "Ordinary test records must not imply an observed retry count"
    )


def test_action_pins_require_native_download_records_and_matching_shas() -> None:
    """Keep every distinct immutable reference from runner download records."""
    checkout_sha = "3d3c42e5aac5ba805825da76410c181273ba90b1"
    shared_action_shas = (
        "6cec89bac47a21cf756d68d638a9a510998e57f8",
        "7d751eded578e48dacf43ab173d5820e6437c474",
    )
    evidence = parse_log(
        "\n".join([
            (
                "2026-10-12T00:00:00Z Download action repository "
                f"'actions/checkout@{checkout_sha}' (SHA:{checkout_sha})"
            ),
            *(
                "2026-10-12T00:00:01Z Download action repository "
                f"'leynos/shared-actions@{sha}' (SHA:{sha})"
                for sha in shared_action_shas
            ),
        ])
    )

    expected = {f"actions/checkout@{checkout_sha}": checkout_sha}
    expected.update({f"leynos/shared-actions@{sha}": sha for sha in shared_action_shas})
    assert evidence["action_pins"] == expected, (
        "Each immutable download reference must retain its SHA"
    )


def test_action_pins_ignore_git_dependencies_and_workflow_references() -> None:
    """Do not turn pinned Git inputs or `uses` lines into action evidence."""
    sha = "3754876bc4e94dc65c83af3def37828934f8f65e"
    evidence = parse_log(
        "\n".join([
            f"Updating https://github.com/leynos/shared-actions ({sha})",
            (
                "Building df12-python-lints @ "
                "git+https://github.com/leynos/df12-python-lints.git@"
                "4cf41736cce2f7ba2778882a5c629c044568a0e5"
            ),
            f"##[group]Run leynos/shared-actions/.github/actions/setup-rust@{sha}",
            (f"Download action repository 'actions/checkout@v4' (SHA:{sha})"),
            (
                "Download action repository 'actions/checkout@"
                f"{sha}' (SHA:4cf41736cce2f7ba2778882a5c629c044568a0e5)"
            ),
            (f"##[debug]Download action repository 'actions/cache@{sha}' (SHA:{sha})"),
        ])
    )

    assert not evidence["action_pins"], (
        "Only complete native download records with immutable refs are action pins"
    )


def test_runner_image_facts_are_scoped_to_the_runner_image_group() -> None:
    """Ignore similarly named fields outside the Runner Image log group."""
    evidence = parse_log(
        "\n".join([
            "##[group]Runner Image Provisioner",
            "Version: provisioner-1",
            "Arch: provisioner-arch",
            "##[endgroup]",
            "##[group]Runner Image",
            "Image: ubuntu-24.04",
            "Version: 20261015.1.0",
            "Arch: X64",
            "##[endgroup]",
            "Image: spoofed-image",
            "Version: unrelated-tool-2",
            "Arch: spoofed-arch",
        ])
    )

    expected_runner = {
        "image": "ubuntu-24.04",
        "image_version": "20261015.1.0",
        "architecture": "X64",
        "cpu_model": None,
        "memory_capacity_mib": None,
    }
    assert evidence["runner"] == expected_runner, (
        "Only Runner Image group fields can identify the runner provenance"
    )


def test_coverage_environment_uses_only_the_current_run_step() -> None:
    """Do not attribute an earlier step's flags to coverage execution."""
    evidence = parse_log(
        "\n".join([
            "##[group]Run build setup",
            "  RUSTFLAGS: -D warnings -Zthreads=8",
            "##[endgroup]",
            "##[group]Run coverage",
            "  NEXTEST_PROFILE: ci",
            "$ cargo llvm-cov nextest --workspace --all-features",
            "##[endgroup]",
        ])
    )

    assert evidence["coverage_environment"] == {"NEXTEST_PROFILE": "ci"}, (
        "Coverage settings must come from the step running the coverage command"
    )
    assert evidence["environment"] == {
        "RUSTFLAGS": "-D warnings -Zthreads=8",
        "NEXTEST_PROFILE": "ci",
    }, "The general environment inventory must retain observations from both steps"


@pytest.mark.parametrize(
    "case",
    [
        (
            "Starting 1 test across 1 binary",
            ["PASS [ 0.100s] netsuke::unit works"],
            "Summary [ 0.100s] 1 test run: 1 passed",
            1,
            0,
            {"passed": 1},
        ),
        (
            "Starting 2 tests across 1 binaries (1 tests skipped)",
            [
                "PASS [ 0.100s] (1/2) netsuke::unit works",
                "FAIL [ 0.200s] (2/2) netsuke::unit fails",
            ],
            "Summary [ 0.300s] 2 tests run: 1 passed, 1 failed, 1 skipped",
            2,
            1,
            {"passed": 1, "failed": 1, "skipped": 1},
        ),
    ],
)
def test_nextest_summaries_accept_singular_and_plural_forms(
    case: tuple[str, list[str], str, int, int, dict[str, int]],
) -> None:
    """Preserve one-test and multi-test Nextest summaries and inventories."""
    start, test_lines, summary, executed, skipped, outcomes = case
    started_at = "2026-10-12T00:00:00Z"
    completed_at = "2026-10-12T00:00:01Z"
    evidence = parse_log(
        "\n".join([
            f"{started_at} {start}",
            *test_lines,
            f"{completed_at} {summary}",
        ])
    )
    assert evidence["nextest"]["executed_count"] == executed, (
        "Nextest summary count must accept either grammatical number"
    )
    assert evidence["nextest"]["outcomes"] == outcomes, (
        "Summary outcomes must remain complete"
    )
    assert evidence["nextest"]["skipped_count"] == skipped, (
        "An absent skipped-count suffix must mean zero skipped tests"
    )
    assert evidence["nextest"]["executed_identity_inventory_complete"] is True, (
        "One-test and multi-test summaries must reconcile with test identities"
    )
    phases = evidence["native_phases"]
    assert len(phases) == 2, "Both Nextest phase markers must be retained"
    assert phases[0]["phase"] == "nextest_started", (
        "The start marker must share the singular/plural Nextest pattern"
    )
    assert phases[0]["timestamp"] == started_at, (
        "The start marker must retain its timestamp"
    )
    assert phases[1]["phase"] == "nextest_completed", (
        "The completion marker must remain distinct from the start marker"
    )
    assert phases[1]["timestamp"] == completed_at, (
        "The completion marker must retain its timestamp"
    )


def test_nextest_discards_repeated_failures_after_summary_without_start() -> None:
    """Preserve a pre-summary test when logs omit Starting and repeat failures."""
    evidence = parse_log(
        "\n".join([
            "2026-10-12T00:00:00Z FAIL [ 0.200s] fixture::unit test_fails",
            "2026-10-12T00:00:01Z Summary [ 0.300s] 1 test run: 1 failed",
            "2026-10-12T00:00:02Z FAIL [ 0.300s] fixture::unit test_fails",
        ])
    )
    nextest = evidence["nextest"]
    assert nextest["executed_count"] == 1, "The summary must retain one executed test"
    assert len(nextest["tests"]) == nextest["executed_count"], (
        "Repeated failures after Summary must not duplicate test outcomes"
    )
    assert nextest["tests"][0]["outcome"] == "fail", (
        "The original failed test outcome must remain"
    )
    assert nextest["tests"][0]["ended_at"] == "2026-10-12T00:00:00Z", (
        "The duplicate summary-time line must not replace the original timestamp"
    )


def test_nextest_retries_retain_only_final_outcome_and_observed_count() -> None:
    """Group TRY records by identity and count retry attempts from their markers."""
    evidence = parse_log(
        "\n".join([
            "Starting 1 test across 1 binary",
            "TRY 1 FAIL [ 0.003s] fixture::unit test_flaky",
            "TRY 2 PASS [ 0.002s] fixture::unit test_flaky",
            "Summary [ 0.021s] 1 test run: 1 passed (1 flaky)",
            "FLAKY 2/3 [ 0.002s] fixture::unit test_flaky",
        ])
    )
    nextest = evidence["nextest"]
    assert len(nextest["tests"]) == 1, (
        "Attempts for one identity must produce one result"
    )
    assert nextest["tests"][0]["outcome"] == "pass", (
        "The last observed retry determines the retained outcome"
    )
    assert nextest["tests"][0]["seconds"] == pytest.approx(0.002), (
        "The retained duration must belong to the final retry"
    )
    assert nextest["retries"] == 1, "Only the observed second attempt is a retry"
    assert nextest["executed_identity_inventory_complete"] is True, (
        "Retry attempts must still reconcile to one executed test"
    )


def test_log_export_never_copies_raw_errors_or_secrets() -> None:
    """Export only recognized fields, not arbitrary log lines or JSON keys."""
    evidence = parse_log(
        "2026-10-12T00:00:00Z token=SUPERSECRET error: private\n"
        '2026-10-12T00:00:01Z {"stats":{"compile_requests":4},"token":"SUPERSECRET"}'
    )
    assert "SUPERSECRET" not in json.dumps(evidence), (
        "Private log text must never enter published evidence"
    )
    assert evidence["sccache"]["compile_requests"] == 4, (
        "Allowlisted native counters must survive redaction"
    )


def test_child_harness_results_are_not_counted_as_doctests() -> None:
    """Only associate result lines with an observed Doc-tests section."""
    evidence = parse_log(
        "test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out"
    )
    assert evidence["doctests"]["counts"] is None, (
        "Child harness output must not inflate doctest counts"
    )


def test_fallback_native_key_is_retained() -> None:
    """A prefix restore is not an exact hit or a cold miss."""
    evidence = parse_log(
        "Cache restored from key: netsuke-cargo-v1-old\n"
        "- registry key: `netsuke-cargo-v1-new` hit: `false`"
    )
    assert evidence["caches"]["registry"]["state"] == "fallback", (
        "A native prefix restore must remain a fallback"
    )
    assert evidence["caches"]["registry"]["matched_key"] == "netsuke-cargo-v1-old", (
        "The actual restored key must remain available"
    )


def test_native_miss_includes_restore_prefix() -> None:
    """Retain a definite miss when the cache action prints multiple input keys."""
    evidence = parse_log(
        "Cache not found for input keys: netsuke-cargo-v1-new, netsuke-cargo-v1-\n"
        "- registry key: `netsuke-cargo-v1-new` hit: `false`"
    )
    assert evidence["caches"]["registry"]["state"] == "miss", (
        "Multiple requested keys must not obscure a native cache miss"
    )


def test_native_archive_bytes_are_bound_to_restored_key() -> None:
    """Use the native byte size, not a rounded MB figure or another archive."""
    evidence = parse_log(
        "Cache Size: ~1 MB (1234567 B)\n"
        "Cache restored from key: netsuke-cargo-v1-old\n"
        "- registry key: `netsuke-cargo-v1-new` hit: `false`"
    )
    assert evidence["caches"]["registry"]["archive_bytes"] == 1234567, (
        "Archive size must use native bytes for the restored key"
    )


def test_native_cargo_and_ratchet_summaries_keep_their_limits() -> None:
    """Cargo elapsed and headline coverage do not prove rebuilds or fidelity."""
    evidence = parse_log(
        "    Finished `test` profile [unoptimized] target(s) in 1m 30s\n"
        "Current coverage: 92.39%\nBaseline coverage: 92.0%"
    )
    assert evidence["build_profile_durations"][0]["seconds"] == 90, (
        "Cargo minute and second components must both be retained"
    )
    assert evidence["nested_cargo"]["rebuilt_artifacts"] is None, (
        "Cargo elapsed time must not imply real recompilation"
    )
    assert evidence["coverage"]["current_percent"] == pytest.approx(92.39), (
        "The current ratchet percentage must be preserved"
    )
    assert evidence["coverage"]["total_lines"] is None, (
        "Headline coverage must not imply a known denominator"
    )
    assert evidence["coverage"]["detailed_report_sha256"] is None, (
        "Headline coverage must not imply a detailed report digest"
    )
