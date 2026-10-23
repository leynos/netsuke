"""Exercise the release observability script as the workflow invokes it."""

import json
from pathlib import Path

from release_operation_observability import main


def test_main_writes_summary_and_telemetry(tmp_path: Path) -> None:
    """Write both workflow outputs using an injected environment mapping."""
    summary_path = Path(tmp_path, "summary.md")
    telemetry_path = Path(tmp_path, "release-operation-observability.jsonl")
    environment = {
        "RELEASE_OPERATION": "release_staging",
        "RELEASE_DRY_RUN": "true",
        "JOB_STATUS": "success",
        "UPLOAD_ERROR_PRESENT": "false",
        "GITHUB_STEP_SUMMARY": str(summary_path),
        "OBSERVABILITY_PATH": str(telemetry_path),
    }

    main(environment)
    summary = summary_path.read_text(encoding="utf-8")
    assert "- Operation: `release_staging`" in summary, (
        "the script must append its operation summary"
    )
    records = [
        json.loads(line)
        for line in telemetry_path.read_text(encoding="utf-8").splitlines()
    ]
    assert records, "the script must write telemetry records"
    assert all(record.get("operation") == "release_staging" for record in records), (
        "the script must write telemetry for the selected operation"
    )
