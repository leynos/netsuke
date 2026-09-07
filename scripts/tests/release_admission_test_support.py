"""Provide reusable subprocess fakes for release-admission runtime tests."""

import dataclasses
import importlib.util
import json
import os
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the script boundary is under test.
import typing as typ
from pathlib import Path

from release_admission_test_fakes import write_fake_commands
from release_admission_test_records import (
    assert_failure_trace_sequence,
    operation_duration,
    operation_records,
)

__all__ = (
    "BASH_PATH",
    "CANARY_BY_OPERATION",
    "METRICS_VALIDATOR",
    "REVISION",
    "FailureCase",
    "_run_gate",
    "assert_failure_trace_sequence",
    "expected_gate_labels",
    "expected_operation_labels",
    "operation_duration",
    "operation_records",
)

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


@dataclasses.dataclass(frozen=True, slots=True)
class FailureCase:
    """Describe one fixed release-admission failure classification.

    Attributes
    ----------
    evidence_state
        Evidence state supplied to the admission subprocess.
    extra_environment
        Additional environment values used to configure a fake boundary.
    operation
        Operation expected to classify the failure.
    error_category
        Bounded category expected for the failure.
    enforce
        Whether the subprocess runs in enforcement mode.

    Notes
    -----
    Contract invariants: operation and error category are fixed vocabulary
    members; ``extra_environment`` applies only to the child process.
    """

    evidence_state: str
    extra_environment: dict[str, str]
    operation: str
    error_category: str
    enforce: bool = True


class MetricsValidator(typ.Protocol):
    """Define finite-JSON parsing and fixed-label validation operations.

    Notes
    -----
    Contract invariants: implementations enforce exact schemas, ordered
    fields, bounded vocabularies, and finite JSON values.
    """

    def parse_metrics(self, lines: list[str]) -> list[dict[str, object]]:
        """Parse finite JSON Lines metric records into mappings.

        Parameters
        ----------
        lines
            JSON Lines containing release-admission metric records.

        Returns
        -------
        list[dict[str, object]]
            Decoded metric objects in input order.
        """

    def validate_metrics(self, records: list[dict[str, object]]) -> None:
        """Validate records against the fixed metric contract.

        Parameters
        ----------
        records
            Decoded metric records to validate.

        Returns
        -------
        None
            The method returns after all records satisfy the contract.
        """

    def parse_traces(self, lines: list[str]) -> list[dict[str, object]]:
        """Parse finite JSON Lines release-admission trace records.

        Parameters
        ----------
        lines
            JSON Lines containing release-admission trace records.

        Returns
        -------
        list[dict[str, object]]
            Decoded trace objects in input order.
        """

    def validate_traces(self, records: list[dict[str, object]]) -> None:
        """Validate traces against the fixed trace contract.

        Parameters
        ----------
        records
            Decoded trace records to validate.

        Returns
        -------
        None
            The method returns after all records satisfy the contract.
        """


def load_metrics_validator() -> MetricsValidator:
    """Load the workflow-contract validator without changing the import path.

    Returns
    -------
    MetricsValidator
        Validator implementation loaded from the workflow-contract module.

    Raises
    ------
    AssertionError
        If the workflow-contract module has no importable loader.

    Notes
    -----
    Contract invariants: file loading preserves the repository's single
    validator contract without package installation.
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
    """Return fixed labels for one release-admission operation counter.

    Parameters
    ----------
    canary
        Bounded canary identifier assigned to the operation.
    operation
        Fixed operation name represented by the counter.
    outcome
        Bounded operation outcome.
    error_category
        Bounded error category, or ``"none"`` for success.

    Returns
    -------
    dict[str, str]
        Labels in the operation-counter schema order.

    Notes
    -----
    Contract invariants: return exactly ``canary``, ``operation``, ``outcome``,
    and ``error_category`` for validator vocabulary checks.
    """
    return {
        "canary": canary,
        "operation": operation,
        "outcome": outcome,
        "error_category": error_category,
    }


def expected_gate_labels(outcome: str, error_category: str) -> dict[str, str]:
    """Return fixed labels for the overall release-admission counter.

    Parameters
    ----------
    outcome
        Bounded overall gate outcome.
    error_category
        Bounded error category, or ``"none"`` for success.

    Returns
    -------
    dict[str, str]
        Labels in the gate-counter schema order.

    Notes
    -----
    Contract invariants: return exactly ``outcome`` and ``error_category`` for
    validator vocabulary checks.
    """
    return {"outcome": outcome, "error_category": error_category}


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
    """Run the production gate with fakes and return its records and call log.

    Parameters
    ----------
    tmp_path
        Isolated directory for subprocess inputs, outputs, and fakes.
    evidence_state
        Evidence state passed to the admission subprocess.
    extra_environment
        Optional child-process environment overrides for a test scenario.

    Returns
    -------
    tuple
        The completed process, parsed metrics, parsed traces, adapter calls,
        and GitHub output values, in that order.

    Notes
    -----
    Contract invariants: the subprocess receives an isolated environment and
    all returned records are decoded from the files it produced.
    """
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
    """Create the fake-boundary files needed by one subprocess gate run.

    Parameters
    ----------
    tmp_path
        Isolated directory that owns the gate's files.

    Returns
    -------
    dict[str, Path]
        Named paths for fake commands, JSON Lines outputs, and workflow output.

    Notes
    -----
    Contract invariant: every returned path is confined to ``tmp_path``.
    """
    fake_bin = tmp_path / "fake-bin"
    fake_bin.mkdir(parents=True)
    call_log = write_fake_commands(fake_bin)
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
    """Build an isolated environment for one real shell gate invocation.

    Parameters
    ----------
    tmp_path
        Isolated directory used for runner-local output paths.
    evidence_state
        Evidence state exposed to the child process.
    extra_environment
        Optional environment overrides for the scenario under test.
    paths
        Named fake-boundary paths returned by :func:`_gate_paths`.

    Returns
    -------
    dict[str, str]
        Environment mapping passed to the admission subprocess.

    Notes
    -----
    Contract invariants: production environment values are preserved unless
    explicitly overridden for the child process.
    """
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
    """Read parsed metric, trace, and fake-boundary records from one run.

    Parameters
    ----------
    paths
        Named output paths returned by :func:`_gate_paths`.

    Returns
    -------
    tuple
        Parsed metric records, parsed trace records, and fake adapter calls.

    Notes
    -----
    Contract invariant: JSON Lines are decoded through the shared validator so
    runtime tests exercise the same schema as workflow-contract tests.
    """
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
