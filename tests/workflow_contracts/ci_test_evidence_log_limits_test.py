"""Reject oversized native log values and recover from malformed sccache JSON."""

import pytest
from ci_test_evidence_logs import parse_log


@pytest.mark.parametrize(
    "line",
    [
        '{"stats":{"compile_requests":' + ("9" * 5000) + "}}",
        '{"stats":' + ("[" * 2000) + "0" + ("]" * 2000) + "}",
    ],
    ids=("oversized-integer", "deep-nesting"),
)
def test_malformed_sccache_json_does_not_abort_log_parsing(line: str) -> None:
    """Leave sccache fields unknown when JSON decoding exceeds its limits."""
    evidence = parse_log(line)

    assert evidence["sccache"]["compile_requests"] is None, (
        "Undecodable sccache counters must remain unknown"
    )
    assert evidence["versions"]["sccache"] is None, (
        "Undecodable sccache versions must remain unknown"
    )


def test_oversized_sccache_json_counter_remains_unknown() -> None:
    """Keep JSON counters within the same bound as text counters."""
    evidence = parse_log('{"stats":{"compile_requests":1000000000000000000}}')

    assert evidence["sccache"]["compile_requests"] is None, (
        "A 19-digit sccache counter must remain unknown"
    )


def test_oversized_native_counts_and_sizes_remain_unknown() -> None:
    """Ignore digit runs beyond the bounded native evidence format."""
    oversized = "9" * 5000
    evidence = parse_log(
        "\n".join([
            f"Cache Size: ~1 MB ({oversized} B)",
            "Cache restored from key: netsuke-cargo-v1-old",
            "- registry key: `netsuke-cargo-v1-old` hit: `true`",
            f"Cache hits {oversized}",
            f"- memory: {oversized} MiB on 4 vCPUs",
            f"- disk used: 1 MiB, least free: {oversized} MiB",
            "Doc-tests netsuke",
            (
                f"test result: ok. {oversized} passed; 0 failed; 0 ignored; "
                "0 measured; 0 filtered out"
            ),
        ])
    )

    assert evidence["caches"]["registry"]["archive_bytes"] is None, (
        "An oversized cache archive size must remain unknown"
    )
    assert evidence["sccache"]["hits"] is None, (
        "An oversized sccache counter must remain unknown"
    )
    assert evidence["resources"]["system_used_peak_mib"] is None, (
        "An oversized memory value must remain unknown"
    )
    assert evidence["resources"]["disk_least_free_mib"] is None, (
        "An oversized disk value must remain unknown"
    )
    assert evidence["doctests"]["counts"] is None, (
        "An oversized doctest result must not create a count section"
    )
