"""Run the release-admission gate and decode the records one run produced.

This module owns the subprocess boundary the release-admission runtime tests
exercise: it builds an isolated environment, invokes the gate under test, and
returns the completed process alongside the three artefact sets it wrote. The
frozen measurements those tests are built on live in
:mod:`release_admission_test_support`, which re-exports everything here as the
facade the runtime modules import from, and the doubles the child reaches are
registered by :mod:`release_admission_test_doubles`.

The gate is a real process because only a process exercises the whole contract:
the shebang ``uv`` honours, the exit status the workflow reads, the artefact
files the gate creates for itself, and the redirections it performs. The
executables it runs are cmd-mox shims, so the boundary under test is the one
production reaches while every external effect stays inside the test.
"""

import dataclasses
import importlib.util
import os
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the script boundary is under test.
import typing as typ
from pathlib import Path

from release_admission_test_doubles import (
    GITHUB_REPOSITORY,
    PYTHON_PATH,
    REVISION,
    SCRIPT_PATH,
    GateRun,
)

if typ.TYPE_CHECKING:
    from cmd_mox import CmdMox

REPO_ROOT = Path(__file__).resolve().parents[2]
METRICS_VALIDATOR_PATH = (
    REPO_ROOT / "tests" / "workflow_contracts" / "release_admission_metrics.py"
)


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


def run_gate(
    cmd_mox: CmdMox,
    tmp_path: Path,
    *,
    evidence_state: str = "missing",
    extra_environment: dict[str, str] | None = None,
) -> GateRun:
    """Run the production gate under cmd-mox and collect everything it wrote.

    Parameters
    ----------
    cmd_mox
        Active controller, already in replay, whose shims are on the child
        ``PATH``.
    tmp_path
        Isolated directory for inputs and outputs.
    evidence_state
        Evidence state passed to the admission subprocess.
    extra_environment
        Optional child-process environment overrides for a test scenario.

    Returns
    -------
    GateRun
        The completed process and the records decoded from the files it wrote.

    Notes
    -----
    Contract invariants: the child receives an isolated environment, the only
    ``gh`` and ``git`` it can reach are the shims, its standard input is closed,
    and every returned record is decoded from a file the gate itself produced.
    """
    paths = _gate_paths(tmp_path)
    environment = _gate_environment(
        cmd_mox, GateInputs(tmp_path, evidence_state, extra_environment, paths)
    )
    result = subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - fixed test target.
        [str(PYTHON_PATH), str(SCRIPT_PATH)],
        capture_output=True,
        check=False,
        env=environment,
        stdin=subprocess.DEVNULL,
        text=True,
    )
    metrics, traces = _read_gate_records(paths)
    outputs = dict(
        line.split("=", maxsplit=1)
        for line in paths["output"].read_text(encoding="utf-8").splitlines()
    )
    return GateRun(result, metrics, traces, outputs, paths)


def _gate_paths(tmp_path: Path) -> dict[str, Path]:
    """Name the files one gate run writes into.

    Parameters
    ----------
    tmp_path
        Isolated directory that owns the gate's files.

    Returns
    -------
    dict[str, Path]
        Named paths for the JSON Lines outputs and the workflow output.

    Notes
    -----
    Contract invariant: every returned path is confined to ``tmp_path``.
    """
    return {
        "metrics": tmp_path / "release-admission-metrics.jsonl",
        "trace": tmp_path / "release-admission-traces.jsonl",
        "output": tmp_path / "github-output",
    }


@dataclasses.dataclass(frozen=True, slots=True)
class GateInputs:
    """Hold the runner-local inputs one gate invocation is configured from.

    These four values travel together -- the directory everything is confined
    to, the state the fixture supplies, the case's own overrides, and the named
    output paths -- so they are one value rather than a five-argument call.
    """

    tmp_path: Path
    evidence_state: str
    extra_environment: dict[str, str] | None
    paths: dict[str, Path]


def _gate_environment(
    cmd_mox: CmdMox,
    inputs: GateInputs,
) -> dict[str, str]:
    """Build an isolated environment for one real gate invocation.

    The PATH is the controller's own: cmd-mox prepends its shim directory when
    it enters, so the copy made here is what puts the shims ahead of any real
    ``gh`` or ``git`` the host happens to have. A scenario must never replace
    that PATH. A shim is a Python script whose shebang resolves ``python3``
    through the child's own PATH, so a child started without one cannot run the
    shim at all -- it fails before it can record anything, which reads as an
    unexplained missing invocation rather than as the configuration error it is.

    Parameters
    ----------
    cmd_mox
        Active controller whose shim directory leads the child PATH.
    inputs
        Runner-local inputs: the isolated directory, the fixture state, the
        case's own overrides, and the named output paths.

    Returns
    -------
    dict[str, str]
        Environment mapping passed to the admission subprocess.

    Notes
    -----
    Contract invariants: the child inherits the controller's environment, so
    the shim directory and the IPC socket reach it, and production values are
    preserved unless a scenario overrides them. An absent shim directory means
    this helper was called outside a live controller, which is a fault in the
    test rather than a condition to report.
    """
    shim_dir = cmd_mox.environment.shim_dir
    assert shim_dir is not None, "CmdMox must create its command shim directory"
    paths = inputs.paths
    environment = {
        **os.environ,
        "GITHUB_OUTPUT": str(paths["output"]),
        "GITHUB_REPOSITORY": GITHUB_REPOSITORY,
        "GITHUB_SHA": REVISION,
        "NETSUKE_RELEASE_ADMISSION_EVIDENCE_STATE": inputs.evidence_state,
        "NETSUKE_RELEASE_ADMISSION_METRICS_FILE": str(paths["metrics"]),
        "NETSUKE_RELEASE_ADMISSION_TRACE_FILE": str(paths["trace"]),
        "PATH": os.pathsep.join([str(shim_dir), os.environ["PATH"]]),
        "RUNNER_TEMP": str(inputs.tmp_path),
    }
    if inputs.extra_environment is not None:
        environment.update(inputs.extra_environment)
    return environment


def _read_gate_records(
    paths: dict[str, Path],
) -> tuple[list[dict[str, object]], list[dict[str, object]]]:
    """Read parsed metric and trace records from one run.

    Parameters
    ----------
    paths
        Named output paths returned by :func:`_gate_paths`.

    Returns
    -------
    tuple
        Parsed metric records and parsed trace records.

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
    return metrics, traces


__all__ = (
    "GITHUB_REPOSITORY",
    "METRICS_VALIDATOR",
    "METRICS_VALIDATOR_PATH",
    "PYTHON_PATH",
    "REPO_ROOT",
    "REVISION",
    "SCRIPT_PATH",
    "GateRun",
    "MetricsValidator",
    "load_metrics_validator",
    "run_gate",
)
