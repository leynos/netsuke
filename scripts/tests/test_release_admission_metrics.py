"""Exercise bounded release-admission metrics through fake command adapters."""

import dataclasses
import importlib.util
import json
import os
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the script boundary is under test.
import typing as typ
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT_PATH = (
    REPO_ROOT / ".github" / "scripts" / "require-release-admission-canaries.sh"
)
METRICS_VALIDATOR_PATH = (
    REPO_ROOT / "tests" / "workflow_contracts" / "release_admission_metrics.py"
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
SUCCESS_GATE_OUTPUTS = {
    "gate-outcome": "success",
    "gate-error-category": "none",
}


@dataclasses.dataclass(frozen=True, slots=True)
class FailureCase:
    """Describe one fixed release-admission failure classification."""

    evidence_state: str
    extra_environment: dict[str, str]
    operation: str
    error_category: str


class MetricsValidator(typ.Protocol):
    """Define finite-JSON parsing and fixed-label validation operations."""

    def parse_metrics(self, lines: list[str]) -> list[dict[str, object]]:
        """Parse finite JSON Lines metric records into mappings."""

    def validate_metrics(self, records: list[dict[str, object]]) -> None:
        """Validate that records retain the fixed release-admission contract."""


def load_metrics_validator() -> MetricsValidator:
    """Load the workflow-contract validator without changing the import path.

    Returns
    -------
    MetricsValidator
        The validator enforcing finite JSON and bounded labels.

    Raises
    ------
    AssertionError
        If the validator module cannot be loaded.
    """
    specification = importlib.util.spec_from_file_location(
        "release_admission_metrics_contract", METRICS_VALIDATOR_PATH
    )
    if specification is None or specification.loader is None:
        message = "the release-admission metrics validator must be loadable"
        raise AssertionError(message)
    module = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    return typ.cast("MetricsValidator", module)


METRICS_VALIDATOR = load_metrics_validator()


def expected_operation_labels(
    canary: str, operation: str, outcome: str, error_category: str
) -> dict[str, str]:
    """Return the fixed-label metric contract for one operation counter.

    Parameters
    ----------
    canary, operation, outcome, error_category
        Fixed bounded values for the operation counter.

    Returns
    -------
    dict[str, str]
        Ordered counter labels; no unbounded dimension is accepted.
    """
    return {
        "canary": canary,
        "operation": operation,
        "outcome": outcome,
        "error_category": error_category,
    }


def expected_gate_labels(outcome: str, error_category: str) -> dict[str, str]:
    """Return the fixed-label metric contract for the overall gate counter.

    Parameters
    ----------
    outcome, error_category
        Fixed bounded values for the gate counter.

    Returns
    -------
    dict[str, str]
        Ordered gate labels without operation identifiers.
    """
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
if [[ -n "${NETSUKE_FAKE_GH_DELAY_SECONDS:-}" ]]; then
  sleep "$NETSUKE_FAKE_GH_DELAY_SECONDS"
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
    subprocess.CompletedProcess[str],
    list[dict[str, object]],
    list[dict[str, object]],
    dict[str, str],
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
    metrics = METRICS_VALIDATOR.parse_metrics(
        metrics_file.read_text(encoding="utf-8").splitlines()
    )
    calls = [
        json.loads(line)
        for line in call_log.read_text(encoding="utf-8").splitlines()
        if line
    ]
    outputs = dict(
        line.split("=", maxsplit=1)
        for line in output_file.read_text(encoding="utf-8").splitlines()
    )
    return result, metrics, calls, outputs


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
    """Verify the successful scaffold emits all required metric records.

    Parameters
    ----------
    tmp_path
        Isolated fake-command and output directory.

    Notes
    -----
    Every fixed operation, duration, gate metric, and summary output is required.
    """
    result, metrics, calls, outputs = _run_gate(tmp_path)

    assert result.returncode == 0, result.stderr
    METRICS_VALIDATOR.validate_metrics(metrics)
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
    assert {
        name: outputs[name] for name in SUCCESS_GATE_OUTPUTS
    } == SUCCESS_GATE_OUTPUTS, "the successful gate must publish its summary outcome"
    assert outputs["metrics-file"] == str(
        tmp_path / "release-admission-metrics.jsonl"
    ), "the successful gate must publish its metrics-file output"


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
        pytest.param(
            FailureCase("unexpected", {}, "check_scan_freshness", "unknown"),
            id="unknown-evidence",
        ),
        pytest.param(
            FailureCase(
                "fresh",
                {
                    "NETSUKE_FAKE_GH_DELAY_SECONDS": "2",
                    "NETSUKE_RELEASE_ADMISSION_OPERATION_TIMEOUT_SECONDS": "1",
                },
                "resolve_tag_commit",
                "timeout",
            ),
            id="operation-timeout",
        ),
    ],
)
def test_gate_emits_fixed_categories_for_failure_paths(
    tmp_path: Path,
    case: FailureCase,
) -> None:
    """Verify each controlled failure retains its bounded metric category.

    Parameters
    ----------
    tmp_path, case
        Isolated output directory and one admission failure variant.

    Notes
    -----
    A failure emits operation, gate, and summary records before a non-zero exit.
    """
    result, metrics, _, outputs = _run_gate(
        tmp_path,
        evidence_state=case.evidence_state,
        extra_environment=case.extra_environment,
    )

    assert result.returncode != 0, "a failed admission operation must block the gate"
    METRICS_VALIDATOR.validate_metrics(metrics)
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
    assert outputs["gate-outcome"] == "failure", (
        "failed operations must reach the workflow summary output"
    )
    assert outputs["gate-error-category"] == case.error_category, (
        "failed operations must retain their bounded category in workflow output"
    )


def test_validator_rejects_non_finite_metric_values() -> None:
    """Verify the JSON contract rejects non-finite metric observations.

    Notes
    -----
    Portable metric JSON excludes Python-specific `Infinity`.
    """
    with pytest.raises(ValueError, match="non-finite JSON metric value: Infinity"):
        METRICS_VALIDATOR.parse_metrics(['{"value": Infinity}'])
