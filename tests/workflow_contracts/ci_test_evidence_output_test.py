"""Specify stable JSONL output and experiment-variant boundaries."""

import json
from pathlib import Path

import pytest
from ci_test_evidence import app, collect_attempt


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
    run, jobs = attempt
    run["repository"] = {"id": 900, "full_name": "leynos/netsuke"}
    run["head_repository"] = {"id": 901, "full_name": "contributor/netsuke"}
    jobs["jobs"].append(
        jobs["jobs"][0]
        | {
            "id": 457,
            "labels": ["ubuntu-latest"],
        }
    )
    jobs["total_count"] = 2
    files = {
        "run": tmp_path / "run.json",
        "jobs": tmp_path / "jobs.json",
        "logs": tmp_path / "logs",
    }
    files["logs"].mkdir()
    files["run"].write_text(json.dumps(run), encoding="utf-8")
    files["jobs"].write_text(json.dumps(jobs), encoding="utf-8")
    raw_log_marker = "raw-log-marker-not-output"
    (files["logs"] / "456.log").write_text(
        f"PRIVATE_TOKEN={raw_log_marker}\n", encoding="utf-8"
    )

    output_lines = []
    for variant in ("control", "telemetry"):
        app(
            [
                "--run",
                str(files["run"]),
                "--jobs",
                str(files["jobs"]),
                "--log-dir",
                str(files["logs"]),
                "--variant",
                variant,
            ],
            result_action="return_value",
        )
        output_lines.extend(capsys.readouterr().out.splitlines())

    records = [json.loads(line) for line in output_lines]
    assert len(records) == 4, "Each run must emit one JSONL record per job"
    assert all(
        line == json.dumps(record, sort_keys=True, separators=(",", ":"))
        for line, record in zip(output_lines, records, strict=True)
    ), "Each output line must remain a compact, key-sorted JSON object"

    schema_fields = sorted(records[0])
    evidence_fields = sorted(records[0]["evidence"])
    assert all(sorted(record) == schema_fields for record in records), (
        "Every JSONL record must use the same schema-2 fields"
    )
    assert all(sorted(record["evidence"]) == evidence_fields for record in records), (
        "Every nested evidence record must use the same fixed fields"
    )
    projection = {
        "schema_fields": schema_fields,
        "evidence_fields": evidence_fields,
        "records": [
            {
                "schema": record["schema"],
                "variant": record["variant"],
                "cohort": record["cohort"],
                "relevant_diff": record["relevant_diff"],
                "tested_merge_sha": record["tested_merge_sha"],
                "runner_labels": record["runner_labels"],
                "outcome": record["outcome"],
                "attempt_outcome": record["attempt_outcome"],
                "queue_and_provisioning_seconds": record[
                    "queue_and_provisioning_seconds"
                ],
                "log_available": record["log_available"],
                "telemetry_available": record["telemetry_available"],
                "cost_basis": record["cost_basis"],
                "billing_consumption": record["billing_consumption"],
                "storage_transfer_bytes": record["storage_transfer_bytes"],
            }
            for record in records
        ],
    }
    snapshot = Path(__file__).parent / "snapshots" / "ci_test_evidence_schema2.json"
    assert projection == json.loads(snapshot.read_text(encoding="utf-8")), (
        "Schema-2 JSONL output must match its stable contract snapshot"
    )
    assert raw_log_marker not in "\n".join(output_lines), (
        "Raw log content must not escape into collected JSONL records"
    )
