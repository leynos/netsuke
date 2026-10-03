"""Run the release-admission gate and decode the records one run produced.

This module owns the subprocess boundary the release-admission runtime tests
exercise: it builds an isolated environment, invokes the gate under test, and
returns the completed process alongside the three artefact sets it wrote. The
frozen measurements those tests are built on live in
:mod:`release_admission_test_support`, which re-exports everything here as the
facade the runtime modules import from.

The isolation is deliberate and narrow. Every path the child touches is
confined to the test's temporary directory, and the fake adapters are put on a
private ``PATH`` prefix, so a run cannot observe or be observed by the host's
own ``gh``, ``git``, or clock. The child process is the only place an
environment is mutated; nothing here changes this process's environment.
"""

import importlib.util
import json
import os
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the script boundary is under test.
import sys
import typing as typ
from pathlib import Path

from release_admission_test_fakes import write_fake_commands

REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT_PATH = REPO_ROOT / ".github" / "scripts" / "release_admission.py"
PYTHON_PATH = Path(sys.executable)
METRICS_VALIDATOR_PATH = (
    REPO_ROOT / "tests" / "workflow_contracts" / "release_admission_metrics.py"
)
REVISION = "a" * 40
GITHUB_REPOSITORY = "leynos/netsuke"


class MetricsValidator(typ.Protocol):
    """Define finite-JSON parsing and fixed-label validation operations.

    Notes
    -----
    Enforce ordered schemas, bounded vocabularies, and finite JSON values.
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
    File loading preserves one validator contract without package installation,
    and keeps the record schema shared with the workflow-contract tests rather
    than restated here.
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
        [str(PYTHON_PATH), str(SCRIPT_PATH)],
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
    return {
        "fake_bin": fake_bin,
        "call_log": call_log,
        "metrics": tmp_path / "release-admission-metrics.jsonl",
        "trace": tmp_path / "release-admission-traces.jsonl",
        "output": tmp_path / "github-output",
    }


def _gate_environment(
    tmp_path: Path,
    evidence_state: str,
    extra_environment: dict[str, str] | None,
    paths: dict[str, Path],
) -> dict[str, str]:
    """Build an isolated environment for one real gate invocation.

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
        "GITHUB_REPOSITORY": GITHUB_REPOSITORY,
        "GITHUB_SHA": REVISION,
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
