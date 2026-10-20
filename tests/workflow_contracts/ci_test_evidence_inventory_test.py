"""Specify attempt-bound inventory validation and privacy guarantees."""

import json
import typing as typ

import pytest
from ci_test_evidence import CollectorInputs, collect_attempt
from ci_test_evidence_logs import parse_log
from ci_test_evidence_telemetry import apply_inventory, load_inventory

if typ.TYPE_CHECKING:
    from pathlib import Path


def _write_inventory(tmp_path: Path, attempt_number: int) -> Path:
    """Write a minimal inventory bound to run 123 and job 456."""
    path = tmp_path / "inventory.json"
    path.write_text(
        json.dumps({
            "schema": 1,
            "run_id": 123,
            "run_attempt": attempt_number,
            "job_id": 456,
        }),
        encoding="utf-8",
    )
    return path


def test_inventory_rejects_other_attempt(tmp_path: Path) -> None:
    """Reject an artefact written by an earlier run attempt."""
    path = _write_inventory(tmp_path, 1)
    with pytest.raises(ValueError, match="inventory attempt mismatch"):
        load_inventory(path, (123, 2, 456))


def test_inventory_accepts_matching_attempt(tmp_path: Path) -> None:
    """Retain the validated attempt identity of a matching artefact."""
    path = _write_inventory(tmp_path, 2)
    assert load_inventory(path, (123, 2, 456))["run_attempt"] == 2, (
        "An inventory must retain its validated attempt identity"
    )


def test_collect_attempt_rejects_inventory_from_another_attempt(
    attempt: tuple[dict, dict],
) -> None:
    """Refuse preloaded telemetry from a different attempt of the same run."""
    run, jobs = attempt
    inventory = {
        "schema": 1,
        "run_id": 123,
        "run_attempt": 1,
        "job_id": 456,
        "profiles": {"raw_count": 2},
    }

    with pytest.raises(ValueError, match="inventory attempt mismatch"):
        collect_attempt(run, jobs, {"inventories": {"456": inventory}})


def test_collect_attempt_applies_matching_inventory(
    attempt: tuple[dict, dict],
) -> None:
    """Copy measurements after in-memory inventory matches the current attempt."""
    run, jobs = attempt
    inventory = {
        "schema": 1,
        "run_id": 123,
        "run_attempt": 2,
        "job_id": 456,
        "profiles": {"raw_count": 2},
    }

    record = collect_attempt(run, jobs, {"inventories": {"456": inventory}})[0]

    assert record["evidence"]["profiles"]["raw_count"] == 2, (
        "Matching telemetry must remain available to offline analysis"
    )


@pytest.mark.parametrize(
    "inventories",
    [None, {"456": []}],
    ids=["inventory-map-is-not-a-mapping", "entry-is-not-an-object"],
)
def test_collect_attempt_rejects_malformed_inventory_input(
    attempt: tuple[dict, dict],
    inventories: object,
) -> None:
    """Reject malformed telemetry maps instead of omitting their evidence."""
    run, jobs = attempt

    with pytest.raises(TypeError, match=r"inventories must be|inventory must be"):
        collect_attempt(
            run, jobs, typ.cast("CollectorInputs", {"inventories": inventories})
        )


@pytest.mark.parametrize("schema", [2, True, None])
def test_inventory_rejects_unknown_schema(tmp_path: Path, schema: object) -> None:
    """Do not read future schemas or accept JSON booleans as version numbers."""
    path = tmp_path / "inventory.json"
    path.write_text(json.dumps({"schema": schema}), encoding="utf-8")
    with pytest.raises(ValueError, match="invalid inventory schema"):
        load_inventory(path, (123, 2, 456))


def test_inventory_rejects_oversized_metadata(tmp_path: Path) -> None:
    """Enforce the metadata envelope before decoding or copying a payload."""
    path = tmp_path / "inventory.json"
    path.write_bytes(b" " * (10 * 1024 * 1024 + 1))
    with pytest.raises(ValueError, match="inventory exceeds metadata budget"):
        load_inventory(path, (123, 2, 456))


def test_inventory_ignores_invalid_numbers_and_private_fields() -> None:
    """Malformed numeric values and free-form fields cannot become evidence."""
    evidence = parse_log("")
    apply_inventory(
        evidence,
        {
            "profiles": {
                "raw_count": 2,
                "raw_bytes": -1,
                "export_seconds": float("nan"),
                "profdata_bytes": 10**1000,
                "object_count": 1.5,
            },
            "resources": {"peak_rss_mib": True},
            "raw_error": "SUPERSECRET",
        },
    )
    assert evidence["profiles"]["raw_count"] == 2, (
        "Valid inventory counts must be retained"
    )
    assert evidence["profiles"]["raw_bytes"] is None, (
        "Negative inventory sizes must remain unknown"
    )
    assert evidence["profiles"]["export_seconds"] is None, (
        "Non-finite durations must remain unknown"
    )
    assert evidence["profiles"]["profdata_bytes"] is None, (
        "Unbounded integer sizes must remain unknown"
    )
    assert evidence["profiles"]["object_count"] is None, (
        "Fractional object counts must remain unknown"
    )
    assert evidence["resources"]["peak_rss_mib"] is None, (
        "Booleans must not become numeric memory measurements"
    )
    assert "SUPERSECRET" not in json.dumps(evidence), (
        "Private inventory fields must never be exported"
    )
