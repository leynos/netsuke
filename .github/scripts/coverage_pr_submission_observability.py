"""Emit bounded metrics and traces for trusted PR coverage submission.

This module writes only fixed-schema JSON Lines records. The workflow uploads
them as trusted observability artefacts; no pull-request text, coverage data,
filesystem paths, or credentials enter either record type.
"""

import argparse
import collections.abc as cabc
import dataclasses
import json
import os
import pathlib
import time

type Environment = cabc.Mapping[str, str]

METRICS_FILE_ENVIRONMENT_KEY = "NETSUKE_CODESCENE_COVERAGE_METRICS_FILE"
TRACES_FILE_ENVIRONMENT_KEY = "NETSUKE_CODESCENE_COVERAGE_TRACES_FILE"
VALID_OPERATIONS = frozenset({
    "coverage-artifact-download",
    "hostile-coverage-validation",
    "codescene-submission",
    "codescene-check-run-publication",
})
VALID_OUTCOMES = frozenset({"success", "failure", "skipped", "unknown"})
ERROR_CATEGORIES = {
    "success": "none",
    "failure": "stage_failure",
    "skipped": "not_run",
    "unknown": "unknown",
}
OPERATION_METRIC_NAME = "netsuke_codescene_pr_coverage_operation_total"
DURATION_METRIC_NAME = "netsuke_codescene_pr_coverage_operation_duration_seconds"
TRACE_EVENT = "operation_complete"


class ObservabilityEnvironmentError(ValueError):
    """Describe a missing or out-of-contract workflow observability value."""


@dataclasses.dataclass(frozen=True, slots=True)
class Telemetry:
    """Hold bounded values shared by one metric and trace record.

    Attributes
    ----------
    operation
        Fixed trusted workflow operation name.
    outcome
        Bounded stage result.
    error_category
        Fixed error classification derived from ``outcome``.
    duration_seconds
        Non-negative elapsed stage duration.
    workflow_run_id
        Originating GitHub Actions workflow run identifier.
    commit_sha
        Originating commit SHA used only as trace correlation context.
    """

    operation: str
    outcome: str
    error_category: str
    duration_seconds: float
    workflow_run_id: str
    commit_sha: str


def _environment_value(environment: Environment, name: str) -> str:
    """Return one required non-empty bounded workflow environment value."""
    value = environment.get(name)
    if not value:
        raise ObservabilityEnvironmentError(name)
    return value


def _append_json_line(path: pathlib.Path, record: dict[str, object]) -> None:
    """Append one compact fixed-schema JSON record to a trusted output file."""
    with path.open("a", encoding="utf-8") as file:
        file.write(json.dumps(record, separators=(",", ":")))
        file.write("\n")


def _write_output(environment: Environment, name: str, value: str) -> None:
    """Write one bounded step output through GitHub Actions' output file."""
    output = pathlib.Path(_environment_value(environment, "GITHUB_OUTPUT"))
    with output.open("a", encoding="utf-8") as file:
        file.write(f"{name}={value}\n")


def _started_at_milliseconds(environment: Environment, ended_at_ms: int) -> int:
    """Return a stage start time or the end time after a skipped prior step."""
    value = environment.get("STARTED_AT_MS")
    if not value:
        return ended_at_ms
    try:
        return int(value)
    except ValueError as error:
        raise ObservabilityEnvironmentError("STARTED_AT_MS") from error


def _outcome_and_error_category(environment: Environment) -> tuple[str, str]:
    """Return a bounded outcome and its fixed error-category mapping."""
    outcome = _environment_value(environment, "OUTCOME")
    bounded_outcome = outcome if outcome in VALID_OUTCOMES else "unknown"
    return bounded_outcome, ERROR_CATEGORIES[bounded_outcome]


def _telemetry_records(
    telemetry: Telemetry,
) -> tuple[dict[str, object], dict[str, object], dict[str, object]]:
    """Build fixed metric and trace records for one trusted workflow stage."""
    operation_labels = {
        "operation": telemetry.operation,
        "outcome": telemetry.outcome,
        "error_category": telemetry.error_category,
    }
    return (
        {"name": OPERATION_METRIC_NAME, "labels": operation_labels, "value": 1},
        {
            "name": DURATION_METRIC_NAME,
            "labels": {"operation": telemetry.operation},
            "value": telemetry.duration_seconds,
        },
        {
            "event": TRACE_EVENT,
            "operation": telemetry.operation,
            "outcome": telemetry.outcome,
            "error_category": telemetry.error_category,
            "duration_seconds": telemetry.duration_seconds,
            "workflow_run_id": telemetry.workflow_run_id,
            "commit_sha": telemetry.commit_sha,
        },
    )


def record_telemetry(
    environment: Environment,
    now_milliseconds: int | None = None,
) -> None:
    """Write and log one trusted coverage metric and trace collection.

    Parameters
    ----------
    environment
        Bounded GitHub Actions stage values and runner-local output paths.
    now_milliseconds
        Optional clock seam for deterministic tests. ``None`` uses the current
        Unix timestamp in milliseconds.

    Raises
    ------
    ObservabilityEnvironmentError
        If a required bounded workflow input is absent or invalid.

    Notes
    -----
    Writes compact JSON Lines metrics and traces to runner-local paths, logs
    the same fixed records, and appends ``duration_ms`` to ``GITHUB_OUTPUT``.
    """
    operation = _environment_value(environment, "OPERATION")
    if operation not in VALID_OPERATIONS:
        raise ObservabilityEnvironmentError("OPERATION")
    ended_at_ms = (
        time.time_ns() // 1_000_000 if now_milliseconds is None else now_milliseconds
    )
    duration_ms = max(
        0, ended_at_ms - _started_at_milliseconds(environment, ended_at_ms)
    )
    outcome, error_category = _outcome_and_error_category(environment)
    workflow_run_id = _environment_value(environment, "ORIGINATING_WORKFLOW_RUN_ID")
    commit_sha = _environment_value(environment, "ORIGINATING_COMMIT_SHA")
    metrics = _telemetry_records(
        Telemetry(
            operation,
            outcome,
            error_category,
            duration_ms / 1000,
            workflow_run_id,
            commit_sha,
        )
    )
    metrics_path = pathlib.Path(
        _environment_value(environment, METRICS_FILE_ENVIRONMENT_KEY)
    )
    traces_path = pathlib.Path(
        _environment_value(environment, TRACES_FILE_ENVIRONMENT_KEY)
    )
    for metric in metrics[:2]:
        _append_json_line(metrics_path, metric)
        print(json.dumps(metric, separators=(",", ":")))
    _append_json_line(traces_path, metrics[2])
    print(json.dumps(metrics[2], separators=(",", ":")))
    _write_output(environment, "duration_ms", str(duration_ms))


def _arguments(argv: cabc.Sequence[str] | None = None) -> argparse.Namespace:
    """Parse the sole fixed observability command."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("record-telemetry",))
    return parser.parse_args(argv)


def main(argv: cabc.Sequence[str] | None = None) -> int:
    """Run the selected trusted coverage observability command.

    Parameters
    ----------
    argv
        Command arguments. ``None`` uses the process arguments.

    Returns
    -------
    int
        ``0`` after writing the fixed metric and trace records.

    Notes
    -----
    Invokes ``record-telemetry`` using the process environment and performs
    only the documented runner-local file and log side effects.
    """
    _arguments(argv)
    record_telemetry(os.environ)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
