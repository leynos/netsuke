"""Specify stable JSONL output and experiment-variant boundaries."""

import json
from pathlib import Path

import pytest
from ci_test_evidence import app, collect_attempt


def _save_cli_attempt(
    attempt: tuple[dict, dict], tmp_path: Path
) -> tuple[Path, Path, Path, str]:
    """Write two-cohort saved inputs and a non-secret raw-log marker."""
    run, jobs = attempt
    run["repository"] = {"id": 900, "full_name": "leynos/netsuke"}
    run["head_repository"] = {"id": 901, "full_name": "contributor/netsuke"}
    jobs["jobs"].append(jobs["jobs"][0] | {"id": 457, "labels": ["ubuntu-latest"]})
    jobs["total_count"] = 2
    run_path = tmp_path / "run.json"
    jobs_path = tmp_path / "jobs.json"
    logs_path = tmp_path / "logs"
    logs_path.mkdir()
    run_path.write_text(json.dumps(run), encoding="utf-8")
    jobs_path.write_text(json.dumps(jobs), encoding="utf-8")
    raw_log_marker = "raw-log-marker-not-output"
    (logs_path / "456.log").write_text(
        f"PRIVATE_TOKEN={raw_log_marker}\n", encoding="utf-8"
    )
    return run_path, jobs_path, logs_path, raw_log_marker


def _collect_variant_output(
    run_path: Path,
    jobs_path: Path,
    logs_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> list[str]:
    """Capture compact JSONL records for both experiment variants."""
    output_lines = []
    for variant in ("control", "telemetry"):
        app(
            [
                "--run",
                str(run_path),
                "--jobs",
                str(jobs_path),
                "--log-dir",
                str(logs_path),
                "--variant",
                variant,
            ],
            result_action="return_value",
        )
        output_lines.extend(capsys.readouterr().out.splitlines())
    return output_lines


def _schema2_projection(records: list[dict]) -> dict[str, object]:
    """Select stable schema and evidence fields for the output snapshot."""
    projected_fields = (
        "schema",
        "variant",
        "cohort",
        "relevant_diff",
        "tested_merge_sha",
        "runner_labels",
        "outcome",
        "attempt_outcome",
        "queue_and_provisioning_seconds",
        "log_available",
        "telemetry_available",
        "cost_basis",
        "billing_consumption",
        "storage_transfer_bytes",
    )
    return {
        "schema_fields": sorted(records[0]),
        "evidence_fields": sorted(records[0]["evidence"]),
        "records": [
            {field: record[field] for field in projected_fields} for record in records
        ],
    }


def _assert_jsonl_shape(output_lines: list[str], records: list[dict]) -> None:
    """Check compact serialization and stable fields across all records."""
    assert len(records) == 4, "Each run must emit one JSONL record per job"
    assert output_lines == [
        json.dumps(record, sort_keys=True, separators=(",", ":")) for record in records
    ], "Each output line must remain a compact, key-sorted JSON object"
    assert {tuple(sorted(record)) for record in records} == {
        tuple(sorted(records[0]))
    }, "Every JSONL record must use the same schema-2 fields"
    assert {tuple(sorted(record["evidence"])) for record in records} == {
        tuple(sorted(records[0]["evidence"]))
    }, "Every nested evidence record must use the same fixed fields"


def _assert_variant_semantics(records: list[dict]) -> None:
    """Check cohort assignment and preserve missing measurements as null."""
    assert [(record["variant"], record["cohort"]) for record in records] == [
        ("control", "pr_coverage"),
        ("control", "fork_hosted"),
        ("telemetry", "pr_coverage"),
        ("telemetry", "fork_hosted"),
    ], "Each variant must retain both cohort assignments"
    assert all(
        record["relevant_diff"] is None
        and record["tested_merge_sha"] is None
        and record["queue_and_provisioning_seconds"] is None
        for record in records
    ), "Missing measurements must remain null"


def test_collect_attempt_rejects_unknown_experiment_variant(
    attempt: tuple[dict, dict],
) -> None:
    """Reject variants outside the fixed experiment vocabulary."""
    run, jobs = attempt

    with pytest.raises(ValueError, match="unknown experiment variant"):
        collect_attempt(run, jobs, {"variant": "untrusted"})


def test_cli_jsonl_snapshot_preserves_schema_and_variants(
    attempt: tuple[dict, dict], tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Keep schema-2 JSONL fields stable across experiment variants."""
    run_path, jobs_path, logs_path, raw_log_marker = _save_cli_attempt(
        attempt, tmp_path
    )
    output_lines = _collect_variant_output(run_path, jobs_path, logs_path, capsys)
    records = [json.loads(line) for line in output_lines]
    _assert_jsonl_shape(output_lines, records)
    _assert_variant_semantics(records)
    snapshot = Path(__file__).parent / "snapshots" / "ci_test_evidence_schema2.json"
    assert _schema2_projection(records) == json.loads(
        snapshot.read_text(encoding="utf-8")
    ), "Schema-2 JSONL output must match its stable contract snapshot"
    assert raw_log_marker not in "\n".join(output_lines), (
        "Raw log content must not escape into collected JSONL records"
    )
