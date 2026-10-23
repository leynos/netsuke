"""Specify version, coverage-selection, and disk facts from native logs."""

import pytest
from ci_test_evidence_logs import parse_log


def test_native_tool_version_records_are_retained() -> None:
    """Keep every documented tool version tied to its native output line."""
    evidence = parse_log(
        "\n".join([
            "rustc 1.91.0-nightly (0123456789 2026-10-01)",
            "cargo 1.91.0-nightly (0123456789 2026-10-01)",
            "cargo-nextest 0.9.112",
            "cargo-llvm-cov 0.9.0 installed to /home/runner/.cargo/bin",
            "LLVM version: 21.1.0",
            '{"stats":{"compile_requests":4},"version":"0.8.9"}',
        ])
    )

    expected_versions = {
        "rustc": "1.91.0-nightly (0123456789 2026-10-01)",
        "cargo": "1.91.0-nightly (0123456789 2026-10-01)",
        "llvm": "21.1.0",
        "nextest": "0.9.112",
        "cargo_llvm_cov": "0.9.0",
        "sccache": "0.8.9",
    }
    assert evidence["versions"] == expected_versions, (
        "Tool versions must retain the native value for each supported tool"
    )
    assert evidence["sccache"]["compile_requests"] == 4, (
        "The version record must retain structured sccache statistics too"
    )


def test_malformed_version_lines_remain_unknown() -> None:
    """Do not infer tool versions from unrelated or malformed log text."""
    evidence = parse_log(
        "\n".join([
            "rustc dev-build",
            "cargo-nextest latest",
            "cargo-llvm-cov unknown installed to /tmp",
            "LLVM version: twenty-one",
            '{"stats":[],"version":"not-a-version"}',
        ])
    )

    assert all(version is None for version in evidence["versions"].values()), (
        "Unsupported version formats must remain explicitly unknown"
    )
    assert evidence["sccache"]["compile_requests"] is None, (
        "Malformed sccache JSON must not produce counters"
    )


@pytest.mark.parametrize(
    ("line", "expected_version"),
    [
        ('{"stats":{"version":"0.8.9"}}', "0.8.9"),
        ('{"stats":{"version":"0.8.9"},"version":42}', None),
    ],
)
def test_sccache_version_uses_present_top_level_value_first(
    line: str, expected_version: str | None
) -> None:
    """Use nested stats only when the top-level version is absent."""
    evidence = parse_log(line)

    assert evidence["versions"]["sccache"] == expected_version, (
        "A malformed top-level version must not fall back to a nested version"
    )


def test_coverage_selection_records_all_flags_and_doctests() -> None:
    """Record all selected coverage flags and the separate doctest pass."""
    evidence = parse_log(
        "\n".join([
            "$ cargo llvm-cov nextest --workspace --all-targets --all-features",
            "$ cargo test --doc --workspace --all-features",
        ])
    )

    expected_selection = {
        "workspace": True,
        "all_targets": True,
        "all_features": True,
        "doctests": True,
    }
    assert evidence["coverage_selection"] == expected_selection, (
        "Coverage and doctest selection must remain separately observable"
    )


def test_unrelated_commands_do_not_supply_coverage_selection() -> None:
    """Keep similarly named flags outside the coverage command out of scope."""
    evidence = parse_log(
        "\n".join([
            "$ cargo test --workspace --all-targets --all-features",
            "cargo llvm-cov nextest --workspace --all-targets --all-features",
            "$ cargo test --workspace --all-targets --all-features --no-run",
        ])
    )

    assert all(value is None for value in evidence["coverage_selection"].values()), (
        "Only observed coverage and doctest commands may populate selection"
    )


def test_mid_line_timestamp_cannot_forge_a_native_test_result() -> None:
    """Leave embedded runner-like text unknown instead of parsing it as a record."""
    evidence = parse_log(
        "child output 2026-10-12T00:00:00Z PASS [ 0.100s] fixture::unit forged"
    )

    assert not evidence["nextest"]["tests"], (
        "A timestamp inside child output must not create a native test identity"
    )
    assert evidence["nextest"]["executed_count"] is None, (
        "An embedded timestamp must not supply a Nextest outcome"
    )


@pytest.mark.parametrize(
    ("line", "used", "free"),
    [
        ("- disk used: 123 MiB, least free: 456 MiB", 123, 456),
        ("disk used: 123 MiB, least free: 456 MiB", None, None),
        ("- disk used: unknown, least free: 456 MiB", None, None),
    ],
)
def test_disk_measurements_require_the_native_resource_record(
    line: str, used: int | None, free: int | None
) -> None:
    """Retain valid disk fields and leave malformed or unscoped values unknown."""
    evidence = parse_log(line)

    assert evidence["resources"]["disk_used_peak_mib"] == used, (
        "Disk usage must come only from the recognized resource record"
    )
    assert evidence["resources"]["disk_least_free_mib"] == free, (
        "Least-free disk must be retained only with a valid usage record"
    )
