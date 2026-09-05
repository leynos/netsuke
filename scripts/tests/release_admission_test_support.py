"""Provide reusable subprocess fakes for release-admission runtime tests."""

import dataclasses
import importlib.util
import json
import os
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the script boundary is under test.
import typing as typ
from pathlib import Path

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
FAKE_ADAPTER_PREAMBLE = """#!/usr/bin/env bash
set -euo pipefail
python3 - "$NETSUKE_ADMISSION_CALL_LOG" ADAPTER_NAME "$@" <<'PY'
import json
import os
import sys

with open(sys.argv[1], "a", encoding="utf-8") as call_log:
    json.dump(
        {
            "command": sys.argv[2],
            "arguments": sys.argv[3:],
            "diagnostics": {
                "path": os.environ.get("NETSUKE_FAKE_PATH", ""),
                "url": os.environ.get("NETSUKE_FAKE_URL", ""),
            },
        },
        call_log,
    )
    call_log.write("\\n")
PY
"""
FAKE_GH_BEHAVIOUR = """if [[ "$*" == *"/actions/runs?"* ]]; then
  gh_failure="${NETSUKE_FAKE_GH_WORKFLOW_FAILURE:-}"
  gh_delay_seconds="${NETSUKE_FAKE_GH_WORKFLOW_DELAY_SECONDS:-}"
  gh_ignores_term="${NETSUKE_FAKE_GH_WORKFLOW_IGNORE_TERM:-}"
else
  gh_failure="${NETSUKE_FAKE_GH_FAILURE:-}"
  gh_delay_seconds="${NETSUKE_FAKE_GH_DELAY_SECONDS:-}"
  gh_ignores_term="${NETSUKE_FAKE_GH_IGNORE_TERM:-}"
fi
if [[ "$gh_failure" == "true" ]]; then exit 1; fi
if [[ "$gh_ignores_term" == "true" ]]; then trap '' TERM; while :; do sleep 1; done; fi
if [[ -n "$gh_delay_seconds" ]]; then sleep "$gh_delay_seconds"; fi
if [[ "$*" == *"/commits/"* ]]; then
  printf '%s\\n' "${NETSUKE_FAKE_RESOLVED_REVISION:-$GITHUB_SHA}"
else
  printf '%s\\n' "${NETSUKE_FAKE_WORKFLOW_RUN_ID-1001}"
fi
"""
FAKE_GIT_BEHAVIOUR = """if [[ "${NETSUKE_FAKE_GIT_FAILURE:-}" == "true" ]]; then
  exit 1
fi
"""


@dataclasses.dataclass(frozen=True, slots=True)
class FailureCase:
    """Describe one fixed release-admission failure classification."""

    evidence_state: str
    extra_environment: dict[str, str]
    operation: str
    error_category: str
    enforce: bool = True


class MetricsValidator(typ.Protocol):
    """Define finite-JSON parsing and fixed-label validation operations."""

    def parse_metrics(self, lines: list[str]) -> list[dict[str, object]]:
        """Parse finite JSON Lines metric records into mappings."""

    def validate_metrics(self, records: list[dict[str, object]]) -> None:
        """Validate that records retain the fixed release-admission contract."""

    def parse_traces(self, lines: list[str]) -> list[dict[str, object]]:
        """Parse finite JSON Lines release-admission trace records."""

    def validate_traces(self, records: list[dict[str, object]]) -> None:
        """Validate that traces retain the fixed release-admission contract."""


def load_metrics_validator() -> MetricsValidator:
    """Load the workflow-contract validator without changing the import path."""
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
    for name, behaviour in (("gh", FAKE_GH_BEHAVIOUR), ("git", FAKE_GIT_BEHAVIOUR)):
        adapter = directory / name
        adapter.write_text(
            FAKE_ADAPTER_PREAMBLE.replace("ADAPTER_NAME", name) + behaviour,
            encoding="utf-8",
        )
        adapter.chmod(0o755)
    return call_log


def _run_gate(
    tmp_path: Path,
    *,
    evidence_state: str = "missing",
    extra_environment: dict[str, str] | None = None,
) -> tuple[
    subprocess.CompletedProcess[str],
    list[dict[str, object]],
    list[dict[str, object]],
    list[dict[str, object]],
    dict[str, str],
]:
    """Run the production gate with fakes and return its records and call log."""
    paths = _gate_paths(tmp_path)
    environment = _gate_environment(tmp_path, evidence_state, extra_environment, paths)
    result = subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - fixed test target.
        [str(BASH_PATH), str(SCRIPT_PATH)],
        capture_output=True,
        check=False,
        env=environment,
        text=True,
    )
    metrics, traces, calls = _read_gate_records(paths)
    outputs = dict(
        line.split("=", maxsplit=1)
        for line in paths["output"].read_text(encoding="utf-8").splitlines()
    )
    return result, metrics, traces, calls, outputs


def _gate_paths(tmp_path: Path) -> dict[str, Path]:
    """Create the fake-boundary files needed by one subprocess gate run."""
    fake_bin = tmp_path / "fake-bin"
    fake_bin.mkdir(parents=True)
    call_log = _write_fake_commands(fake_bin)
    bash_environment = tmp_path / "bash-environment"
    bash_environment.touch()
    return {
        "fake_bin": fake_bin,
        "call_log": call_log,
        "metrics": tmp_path / "release-admission-metrics.jsonl",
        "trace": tmp_path / "release-admission-traces.jsonl",
        "output": tmp_path / "github-output",
        "bash_environment": bash_environment,
    }


def _gate_environment(
    tmp_path: Path,
    evidence_state: str,
    extra_environment: dict[str, str] | None,
    paths: dict[str, Path],
) -> dict[str, str]:
    """Build an isolated environment for one real shell gate invocation."""
    environment = {
        **os.environ,
        "GITHUB_OUTPUT": str(paths["output"]),
        "GITHUB_REPOSITORY": "leynos/netsuke",
        "GITHUB_SHA": REVISION,
        "BASH_ENV": str(paths["bash_environment"]),
        "NETSUKE_ADMISSION_CALL_LOG": str(paths["call_log"]),
        "NETSUKE_RELEASE_ADMISSION_EVIDENCE_STATE": evidence_state,
        "NETSUKE_RELEASE_ADMISSION_METRICS_FILE": str(paths["metrics"]),
        "NETSUKE_RELEASE_ADMISSION_TRACE_FILE": str(paths["trace"]),
        "PATH": f"{paths['fake_bin']}{os.pathsep}{os.environ['PATH']}",
        "RUNNER_TEMP": str(tmp_path),
    }
    if extra_environment is not None:
        environment.update(extra_environment)
    return environment


def _read_gate_records(
    paths: dict[str, Path],
) -> tuple[list[dict[str, object]], list[dict[str, object]], list[dict[str, object]]]:
    """Read parsed metric, trace, and fake-boundary records from one run."""
    metrics = METRICS_VALIDATOR.parse_metrics(
        paths["metrics"].read_text(encoding="utf-8").splitlines()
    )
    traces = METRICS_VALIDATOR.parse_traces(
        paths["trace"].read_text(encoding="utf-8").splitlines()
    )
    calls = [
        json.loads(line)
        for line in paths["call_log"].read_text(encoding="utf-8").splitlines()
        if line
    ]
    return metrics, traces, calls
