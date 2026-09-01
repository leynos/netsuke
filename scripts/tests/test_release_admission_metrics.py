"""Exercise bounded release-admission metrics through fake command adapters."""

import dataclasses
import json
import os
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the script boundary is under test.
import sys
import tempfile
from pathlib import Path

import pytest
from hypothesis import given, settings
from hypothesis import strategies as st

REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "tests" / "workflow_contracts"))

from release_admission_metrics import (  # ruff: ignore[module-import-not-at-top-of-file] - needs path setup
    parse_metrics,
    validate_metrics,
)

SCRIPT_PATH = (
    REPO_ROOT / ".github" / "scripts" / "require-release-admission-canaries.sh"
)
BASH_PATH = Path("/usr/bin/bash")
REVISION = "a" * 40
CANARY_BY_OPERATION = {
    "resolve_tag_commit": "none",
    "fetch_candidate_revision": "release_candidate",
    "fetch_workflow_run": "history_scan",
    "check_scan_freshness": "history_scan",
    "verify_evidence": "history_scan",
}
IDENTIFIER_TEXT = st.text(
    alphabet=st.characters(blacklist_categories=("Cs",), blacklist_characters="\x00"),
    min_size=1,
    max_size=32,
)


@dataclasses.dataclass(frozen=True, slots=True)
class FailureCase:
    """Describe one fixed release-admission failure classification."""

    evidence_state: str
    extra_environment: dict[str, str]
    operation: str
    error_category: str


def expected_operation_labels(
    canary: str, operation: str, outcome: str, error_category: str
) -> dict[str, str]:
    """Return the fixed-label metric contract for one operation counter."""
    return {
        "canary": canary,
        "operation": operation,
        "outcome": outcome,
        "error_category": error_category,
    }


def expected_gate_labels(outcome: str, error_category: str) -> dict[str, str]:
    """Return the fixed-label metric contract for the overall gate counter."""
    return {"outcome": outcome, "error_category": error_category}


def _write_fake_commands(directory: Path) -> Path:
    """Write executable fake GitHub and Git adapters that log every call."""
    call_log = directory / "command-calls.jsonl"
    call_log.touch()
    gh = directory / "gh"
    git = directory / "git"
    gh.write_text(
        """#!/usr/bin/env bash
set -euo pipefail
python3 - "$NETSUKE_ADMISSION_CALL_LOG" gh "$@" <<'PY'
import json
import sys

with open(sys.argv[1], "a", encoding="utf-8") as call_log:
    json.dump({"command": sys.argv[2], "arguments": sys.argv[3:]}, call_log)
    call_log.write("\\n")
PY
if [[ "${NETSUKE_FAKE_GH_FAILURE:-}" == "true" ]]; then
  exit 1
fi
if [[ "$*" == *"/commits/"* ]]; then
  printf '%s\\n' "$GITHUB_SHA"
else
  printf '%s\\n' "${NETSUKE_FAKE_WORKFLOW_RUN_ID:-1001}"
fi
""",
        encoding="utf-8",
    )
    git.write_text(
        """#!/usr/bin/env bash
set -euo pipefail
python3 - "$NETSUKE_ADMISSION_CALL_LOG" git "$@" <<'PY'
import json
import sys

with open(sys.argv[1], "a", encoding="utf-8") as call_log:
    json.dump({"command": sys.argv[2], "arguments": sys.argv[3:]}, call_log)
    call_log.write("\\n")
PY
if [[ "${NETSUKE_FAKE_GIT_FAILURE:-}" == "true" ]]; then
  exit 1
fi
""",
        encoding="utf-8",
    )
    gh.chmod(0o755)
    git.chmod(0o755)
    return call_log


def _run_gate(
    tmp_path: Path,
    *,
    evidence_state: str = "fresh",
    extra_environment: dict[str, str] | None = None,
) -> tuple[
    subprocess.CompletedProcess[str], list[dict[str, object]], list[dict[str, object]]
]:
    """Run the production gate with fakes and return its records and call log."""
    fake_bin = tmp_path / "fake-bin"
    fake_bin.mkdir()
    call_log = _write_fake_commands(fake_bin)
    metrics_file = tmp_path / "release-admission-metrics.jsonl"
    output_file = tmp_path / "github-output"
    bash_environment = tmp_path / "bash-environment"
    bash_environment.touch()
    environment = {
        **os.environ,
        "GITHUB_OUTPUT": str(output_file),
        "GITHUB_REPOSITORY": "leynos/netsuke",
        "GITHUB_SHA": REVISION,
        "BASH_ENV": str(bash_environment),
        "NETSUKE_ADMISSION_CALL_LOG": str(call_log),
        "NETSUKE_RELEASE_ADMISSION_EVIDENCE_STATE": evidence_state,
        "NETSUKE_RELEASE_ADMISSION_METRICS_FILE": str(metrics_file),
        "PATH": f"{fake_bin}{os.pathsep}{os.environ['PATH']}",
        "RUNNER_TEMP": str(tmp_path),
    }
    if extra_environment is not None:
        environment.update(extra_environment)
    result = subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - executes a fixed test target.
        [str(BASH_PATH), str(SCRIPT_PATH)],
        capture_output=True,
        check=False,
        env=environment,
        text=True,
    )
    metrics = parse_metrics(metrics_file.read_text(encoding="utf-8").splitlines())
    calls = [
        json.loads(line)
        for line in call_log.read_text(encoding="utf-8").splitlines()
        if line
    ]
    return result, metrics, calls


def _operation_records(
    metrics: list[dict[str, object]], operation: str
) -> list[dict[str, object]]:
    """Return metric records emitted for the named fixed operation."""
    return [
        record
        for record in metrics
        if record["name"] == "netsuke_release_admission_operation_total"
        and isinstance(record["labels"], dict)
        and record["labels"].get("operation") == operation
    ]


def test_gate_emits_each_fixed_operation_and_a_successful_gate(tmp_path: Path) -> None:
    """The successful scaffold emits counters and durations for every operation."""
    result, metrics, calls = _run_gate(tmp_path)

    assert result.returncode == 0, result.stderr
    validate_metrics(metrics)
    assert {call["command"] for call in calls} == {"gh", "git"}, (
        "the gate must call both GitHub and Git adapters"
    )
    assert len(metrics) == 11, (
        "five operations need a counter and duration plus one gate counter"
    )
    duration_operations = {
        record["labels"]["operation"]
        for record in metrics
        if record["name"] == "netsuke_release_admission_operation_duration_seconds"
        and isinstance(record["labels"], dict)
    }
    assert duration_operations == CANARY_BY_OPERATION.keys(), (
        "each fixed operation must emit one duration metric"
    )
    for operation, canary in CANARY_BY_OPERATION.items():
        records = _operation_records(metrics, operation)
        assert len(records) == 1, f"{operation} must emit one operation counter"
        assert records[0]["labels"] == expected_operation_labels(
            canary, operation, "success", "none"
        ), f"{operation} must report a successful bounded operation counter"
    expected_gate_metric = {
        "name": "netsuke_release_admission_gate_total",
        "labels": expected_gate_labels("success", "none"),
        "value": 1,
    }
    assert metrics[-1] == expected_gate_metric, (
        "the gate must report its successful outcome"
    )


@pytest.mark.parametrize(
    "case",
    [
        pytest.param(
            FailureCase(
                "fresh",
                {"NETSUKE_FAKE_GH_FAILURE": "true"},
                "resolve_tag_commit",
                "api_error",
            ),
            id="api-error",
        ),
        pytest.param(
            FailureCase(
                "fresh",
                {"NETSUKE_FAKE_GIT_FAILURE": "true"},
                "fetch_candidate_revision",
                "fetch_error",
            ),
            id="fetch-error",
        ),
        pytest.param(
            FailureCase("stale", {}, "check_scan_freshness", "stale_evidence"),
            id="stale-evidence",
        ),
        pytest.param(
            FailureCase("missing", {}, "check_scan_freshness", "missing_evidence"),
            id="missing-evidence",
        ),
    ],
)
def test_gate_emits_fixed_categories_for_failure_paths(
    tmp_path: Path,
    case: FailureCase,
) -> None:
    """Each controlled failure fails closed with its documented category."""
    result, metrics, _ = _run_gate(
        tmp_path,
        evidence_state=case.evidence_state,
        extra_environment=case.extra_environment,
    )

    assert result.returncode != 0, "a failed admission operation must block the gate"
    validate_metrics(metrics)
    record = _operation_records(metrics, case.operation)[-1]
    assert record["labels"] == expected_operation_labels(
        CANARY_BY_OPERATION[case.operation],
        case.operation,
        "failure",
        case.error_category,
    ), f"{case.operation} must retain its fixed error category"
    assert metrics[-1]["labels"] == expected_gate_labels(
        "failure", case.error_category
    ), "the gate must retain the operation's error category"


@given(
    revision=IDENTIFIER_TEXT,
    run_id=IDENTIFIER_TEXT,
    path=IDENTIFIER_TEXT,
    url=IDENTIFIER_TEXT,
)
@settings(deadline=None, max_examples=20)
def test_identifiers_never_become_metric_labels(
    revision: str,
    run_id: str,
    path: str,
    url: str,
) -> None:
    """Arbitrary candidate identifiers cannot expand metric cardinality."""
    identifiers = {
        f"revision-{revision}",
        f"run-{run_id}",
        f"path-{path}",
        f"url-{url}",
    }
    with tempfile.TemporaryDirectory() as directory_name:
        result, metrics, _ = _run_gate(
            Path(directory_name),
            extra_environment={
                "GITHUB_SHA": f"revision-{revision}",
                "NETSUKE_FAKE_WORKFLOW_RUN_ID": f"run-{run_id}",
                "NETSUKE_FAKE_PATH": f"path-{path}",
                "NETSUKE_FAKE_URL": f"url-{url}",
            },
        )

    assert result.returncode == 0, result.stderr
    validate_metrics(metrics)
    for record in metrics:
        labels = record["labels"]
        assert isinstance(labels, dict), "every emitted metric must retain labels"
        assert identifiers.isdisjoint(labels.values()), (
            "generated identifiers must never become metric label values"
        )
