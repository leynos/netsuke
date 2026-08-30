"""Test Rustdoc coverage-payload decoding in the Cargo adapter."""

import dataclasses
import typing as typ

import doc_coverage_cargo as cargo_module
import pytest
from hypothesis import given
from hypothesis import strategies as st

if typ.TYPE_CHECKING:
    import types


@dataclasses.dataclass(frozen=True, slots=True)
class CoveragePayloadFailureCase:
    """Define one invalid Rustdoc coverage-payload scenario."""

    payload: str
    diagnostic: str


_COUNT_VALUES = st.one_of(
    st.integers(),
    st.booleans(),
    st.floats(allow_nan=True, allow_infinity=True),
    st.text(),
    st.none(),
)


@given(total=_COUNT_VALUES, with_docs=_COUNT_VALUES)
def test_coverage_entry_accepts_only_valid_count_pairs(
    total: object,
    with_docs: object,
) -> None:
    """Accept only non-negative integer counts with docs no greater than total."""
    entry = {"total": total, "with_docs": with_docs}
    match total, with_docs:
        case bool(), _:
            pass
        case _, bool():
            pass
        case int() as checked_total, int() as checked_with_docs if (
            0 <= checked_with_docs <= checked_total
        ):
            assert cargo_module.coverage_from_entry(entry) == cargo_module.Coverage(
                checked_total, checked_with_docs
            ), (
                "valid count pairs must be converted without changing their "
                f"values; got {entry!r}"
            )
            return

    with pytest.raises(cargo_module.CoverageCountError):
        cargo_module.coverage_from_entry(entry)


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

    assert (coverage.total, coverage.with_docs) == (15, 11), (
        "per-file counts must be summed across the whole payload"
    )


def test_parse_coverage_output_rejects_malformed_json(cargo: types.ModuleType) -> None:
    """Surface non-JSON output as a coverage-gate RuntimeError."""
    target = cargo.DocTarget("netsuke", "lib", None)

    with pytest.raises(cargo.CoverageOutputError, match="did not emit coverage JSON"):
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

    with pytest.raises(
        cargo.CoverageOutputError, match="each entry requires total and with_docs"
    ):
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
        pytest.param(
            CoveragePayloadFailureCase(
                '{"src/lib.rs": ["total", "with_docs"]}',
                "each entry requires total and with_docs",
            ),
            id="non-object-entry",
        ),
    ],
)
def test_parse_coverage_output_rejects_invalid_shape(
    cargo: types.ModuleType,
    case: CoveragePayloadFailureCase,
) -> None:
    """Reject malformed Rustdoc coverage structures as controlled errors."""
    with pytest.raises(cargo.CoverageOutputError, match=case.diagnostic):
        cargo.parse_coverage_output(cargo.DocTarget("x", "lib", None), case.payload)


def test_aggregate_coverage_payload_rejects_non_object(
    cargo: types.ModuleType,
) -> None:
    """Expose a payload-shape error before the adapter translates it."""
    with pytest.raises(cargo.CoveragePayloadShapeError):
        cargo.aggregate_coverage_payload([])


def test_aggregate_coverage_payload_rejects_non_object_entry(
    cargo: types.ModuleType,
) -> None:
    """Expose an entry-shape error before the adapter translates it."""
    with pytest.raises(cargo.CoverageEntryShapeError):
        cargo.aggregate_coverage_payload({"src/lib.rs": []})


def test_aggregate_coverage_payload_rejects_inconsistent_counts(
    cargo: types.ModuleType,
) -> None:
    """Expose a count error before the adapter translates it."""
    with pytest.raises(cargo.CoverageCountError):
        cargo.aggregate_coverage_payload({"src/lib.rs": {"total": 1, "with_docs": 2}})
