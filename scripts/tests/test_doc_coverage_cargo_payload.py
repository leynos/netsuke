"""Test Rustdoc coverage-payload decoding in the Cargo adapter."""

from __future__ import annotations

import dataclasses
import types

import pytest


@dataclasses.dataclass(frozen=True)
class CoveragePayloadFailureCase:
    """Define one invalid Rustdoc coverage-payload scenario."""

    payload: str
    diagnostic: str


def test_parse_coverage_output_aggregates_multiple_files(
    cargo: types.ModuleType,
) -> None:
    """Roll per-file totals and documented counts up across the payload."""
    target = cargo.DocTarget("netsuke", "lib", None)
    payload = (
        '{"src/a.rs": {"total": 10, "with_docs": 8}, '
        '"src/b.rs": {"total": 5, "with_docs": 3}}'
    )

    coverage = cargo.parse_coverage_output(target, payload)

    assert coverage.total == 15
    assert coverage.with_docs == 11


def test_parse_coverage_output_rejects_malformed_json(cargo: types.ModuleType) -> None:
    """Surface non-JSON output as a coverage-gate RuntimeError."""
    target = cargo.DocTarget("netsuke", "lib", None)

    with pytest.raises(RuntimeError, match="did not emit coverage JSON"):
        cargo.parse_coverage_output(target, "not json at all")


@pytest.mark.parametrize(
    "entry",
    [
        pytest.param('{"total": true, "with_docs": 0}', id="boolean"),
        pytest.param('{"total": 1e999, "with_docs": 0}', id="non-finite"),
        pytest.param('{"total": -1, "with_docs": 0}', id="negative"),
        pytest.param('{"total": 1.5, "with_docs": 0}', id="non-integer"),
        pytest.param('{"total": 1, "with_docs": 2}', id="inconsistent"),
    ],
)
def test_parse_coverage_output_rejects_invalid_counts(
    cargo: types.ModuleType,
    entry: str,
) -> None:
    """Reject invalid Rustdoc count invariants as controlled adapter errors."""
    payload = '{"src/lib.rs": ' + entry + "}"

    with pytest.raises(RuntimeError, match="each entry requires total and with_docs"):
        cargo.parse_coverage_output(cargo.DocTarget("x", "lib", None), payload)


@pytest.mark.parametrize(
    "case",
    [
        pytest.param(
            CoveragePayloadFailureCase("[]", "expected an object"),
            id="non-object",
        ),
        pytest.param(
            CoveragePayloadFailureCase(
                '{"src/lib.rs": {"total": 1}}',
                "each entry requires total and with_docs",
            ),
            id="missing-with-docs",
        ),
        pytest.param(
            CoveragePayloadFailureCase(
                '{"src/lib.rs": {"with_docs": 1}}',
                "each entry requires total and with_docs",
            ),
            id="missing-total",
        ),
    ],
)
def test_parse_coverage_output_rejects_invalid_shape(
    cargo: types.ModuleType,
    case: CoveragePayloadFailureCase,
) -> None:
    """Reject malformed Rustdoc coverage structures as controlled errors."""
    with pytest.raises(RuntimeError, match=case.diagnostic):
        cargo.parse_coverage_output(cargo.DocTarget("x", "lib", None), case.payload)
