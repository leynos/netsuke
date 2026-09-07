"""Run trusted CodeScene PR coverage workflow actions with Python only.

The trusted workflow invokes this module through GitHub Actions' ``python``
shell. It accepts fixed commands, emits bounded correlation data, and never
reads pull-request content or artefact members as executable code.
"""

import argparse
import collections.abc as cabc
import importlib.util
import json
import os
import pathlib
import sys
import time
import typing as typ
import urllib.request

type Environment = cabc.Mapping[str, str]
type ValidatorMain = cabc.Callable[[cabc.Sequence[str] | None], int]
type SummaryField = tuple[str, str]

CHECK_RUN_NAME = "CodeScene coverage"
GITHUB_API_VERSION = "2022-11-28"
REPOSITORY_ROOT = pathlib.Path(__file__).resolve().parents[2]
VALIDATOR_PATH = REPOSITORY_ROOT / "scripts" / "validate_coverage_artifact.py"
VALID_OPERATIONS = frozenset({
    "coverage-artifact-download",
    "hostile-coverage-validation",
    "codescene-submission",
    "codescene-check-run-publication",
})
REPORT_FIELDS: tuple[SummaryField, ...] = (
    ("Originating workflow run ID", "workflow_run_id"),
    ("Originating commit SHA", "commit_sha"),
    ("Artifact name", "artifact_name"),
    ("Download outcome", "download_outcome"),
    ("Download duration (ms)", "download_duration_ms"),
    ("Validation outcome", "validation_outcome"),
    ("Validation duration (ms)", "validation_duration_ms"),
    ("Submission outcome", "submission_outcome"),
    ("Submission duration (ms)", "submission_duration_ms"),
)
SUMMARY_FIELDS: tuple[SummaryField, ...] = (
    ("- Originating workflow run ID", "ORIGINATING_WORKFLOW_RUN_ID"),
    ("- Originating commit SHA", "ORIGINATING_COMMIT_SHA"),
    ("- Artifact name", "ARTIFACT_NAME"),
    ("- Download outcome", "ARTIFACT_DOWNLOAD_OUTCOME"),
    ("- Download duration (ms)", "ARTIFACT_DOWNLOAD_DURATION_MS"),
    ("- Validation outcome", "ARTIFACT_VALIDATION_OUTCOME"),
    ("- Validation duration (ms)", "ARTIFACT_VALIDATION_DURATION_MS"),
    ("- Submission outcome", "SUBMISSION_OUTCOME"),
    ("- Submission duration (ms)", "SUBMISSION_DURATION_MS"),
    ("- Check Run publication outcome", "CHECK_RUN_PUBLICATION_OUTCOME"),
    (
        "- Check Run publication duration (ms)",
        "CHECK_RUN_PUBLICATION_DURATION_MS",
    ),
)


class WorkflowEnvironmentError(ValueError):
    """Describe a malformed bounded GitHub Actions environment value."""


class ValidatorLoadError(RuntimeError):
    """Describe an unavailable trusted hostile-artefact validator seam."""


def coverage_conclusion(
    download_outcome: str,
    validation_outcome: str,
    submission_outcome: str,
) -> str:
    """Return the Check Run conclusion for trusted handoff outcomes.

    A skipped submission is neutral only after successful download and
    validation, which represents an intentionally unavailable credential.
    Every other non-success state fails closed.

    Returns
    -------
    str
        The ``success``, ``neutral``, or fail-closed ``failure`` conclusion.
    """
    prerequisites_succeeded = (
        download_outcome == "success" and validation_outcome == "success"
    )
    if submission_outcome == "success":
        return "success"
    if submission_outcome == "skipped" and prerequisites_succeeded:
        return "neutral"
    return "failure"


def _environment_value(environment: Environment, name: str) -> str:
    """Return a required non-empty bounded workflow environment value."""
    value = environment.get(name)
    if not value:
        raise WorkflowEnvironmentError(name)
    return value


def _append_workflow_value(
    environment: Environment, file_name: str, value: str
) -> None:
    """Append one bounded value to a GitHub Actions workflow file."""
    path = pathlib.Path(_environment_value(environment, file_name))
    with path.open("a", encoding="utf-8") as file:
        file.write(value)
        file.write("\n")


def _write_output(environment: Environment, name: str, value: str) -> None:
    """Write one named output through GitHub Actions' output file."""
    _append_workflow_value(environment, "GITHUB_OUTPUT", f"{name}={value}")


def _now_milliseconds() -> int:
    """Return the current UTC-equivalent Unix timestamp in milliseconds."""
    return time.time_ns() // 1_000_000


def start_telemetry(
    environment: Environment,
    now_milliseconds: int | None = None,
) -> None:
    """Record the start time for one bounded trusted workflow stage."""
    started_at_ms = (
        _now_milliseconds() if now_milliseconds is None else now_milliseconds
    )
    _write_output(environment, "started_at_ms", str(started_at_ms))


def _started_at_milliseconds(environment: Environment, ended_at_ms: int) -> int:
    """Return a stage start time or end time when the prior step did not run."""
    value = environment.get("STARTED_AT_MS")
    if not value:
        return ended_at_ms
    try:
        return int(value)
    except ValueError as error:
        raise WorkflowEnvironmentError("STARTED_AT_MS") from error


def record_telemetry(
    environment: Environment,
    now_milliseconds: int | None = None,
) -> None:
    """Emit one fixed-operation telemetry line and its measured duration."""
    operation = _environment_value(environment, "OPERATION")
    if operation not in VALID_OPERATIONS:
        raise WorkflowEnvironmentError("OPERATION")
    ended_at_ms = _now_milliseconds() if now_milliseconds is None else now_milliseconds
    duration_ms = ended_at_ms - _started_at_milliseconds(environment, ended_at_ms)
    outcome = _environment_value(environment, "OUTCOME")
    workflow_run_id = _environment_value(environment, "ORIGINATING_WORKFLOW_RUN_ID")
    commit_sha = _environment_value(environment, "ORIGINATING_COMMIT_SHA")
    _write_output(environment, "duration_ms", str(duration_ms))
    print(
        "coverage-pr-submission "
        f"operation={operation} duration_ms={duration_ms} "
        f"workflow_run_id={workflow_run_id} commit_sha={commit_sha} "
        f"outcome={outcome}"
    )


def _validator_main() -> ValidatorMain:
    """Load the trusted hostile-artefact validator through its explicit seam."""
    spec = importlib.util.spec_from_file_location(
        "validate_coverage_artifact", VALIDATOR_PATH
    )
    if spec is None or spec.loader is None:
        raise ValidatorLoadError
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    validator_main = getattr(module, "main", None)
    if not callable(validator_main):
        raise ValidatorLoadError
    return typ.cast("ValidatorMain", validator_main)


def validate_artefact(artifact_directory: str) -> int:
    """Validate a downloaded artefact as hostile data without a shell.

    Returns
    -------
    int
        The hostile-artefact validator's documented process exit status.
    """
    return _validator_main()(["--artifact-dir", artifact_directory])


def _report_values(environment: Environment) -> dict[str, str]:
    """Collect the bounded correlation values used by a trusted Check Run."""
    environment_names = {
        "workflow_run_id": "ORIGINATING_WORKFLOW_RUN_ID",
        "commit_sha": "ORIGINATING_COMMIT_SHA",
        "artifact_name": "ARTIFACT_NAME",
        "download_outcome": "ARTIFACT_DOWNLOAD_OUTCOME",
        "validation_outcome": "ARTIFACT_VALIDATION_OUTCOME",
        "submission_outcome": "SUBMISSION_OUTCOME",
        "download_duration_ms": "ARTIFACT_DOWNLOAD_DURATION_MS",
        "validation_duration_ms": "ARTIFACT_VALIDATION_DURATION_MS",
        "submission_duration_ms": "SUBMISSION_DURATION_MS",
    }
    return {
        value_name: _environment_value(environment, environment_name)
        for value_name, environment_name in environment_names.items()
    }


def _summary_from_values(
    values: cabc.Mapping[str, str],
    fields: cabc.Iterable[SummaryField],
    conclusion: str,
) -> str:
    """Format fixed labels and bounded values into a Check Run summary."""
    lines = [f"{label}: {values[value_name]}" for label, value_name in fields]
    lines.append(f"Conclusion: {conclusion}")
    return "\n".join(lines)


def _report_summary(values: cabc.Mapping[str, str], conclusion: str) -> str:
    """Format the bounded Check Run summary for a same-repository PR."""
    return _summary_from_values(values, REPORT_FIELDS, conclusion)


def _fork_summary(environment: Environment) -> str:
    """Format the bounded neutral Check Run summary for an excluded fork."""
    values = {
        "workflow_run_id": _environment_value(
            environment, "ORIGINATING_WORKFLOW_RUN_ID"
        ),
        "commit_sha": _environment_value(environment, "ORIGINATING_COMMIT_SHA"),
        "artifact_name": _environment_value(environment, "ARTIFACT_NAME"),
    }
    fields = (
        *REPORT_FIELDS[:3],
        ("Download outcome", "download_outcome"),
        ("Validation outcome", "validation_outcome"),
        ("Submission outcome", "submission_outcome"),
    )
    values.update({
        "download_outcome": "skipped",
        "validation_outcome": "skipped",
        "submission_outcome": "skipped",
    })
    return _summary_from_values(values, fields, "neutral")


def _check_run_payload(
    environment: Environment,
    conclusion: str,
    summary: str,
) -> dict[str, object]:
    """Build the Check Run payload for the originating workflow commit."""
    return {
        "name": CHECK_RUN_NAME,
        "head_sha": _environment_value(environment, "ORIGINATING_COMMIT_SHA"),
        "external_id": _environment_value(environment, "ORIGINATING_WORKFLOW_RUN_ID"),
        "status": "completed",
        "conclusion": conclusion,
        "output": {"title": CHECK_RUN_NAME, "summary": summary},
    }


def _check_run_endpoint(environment: Environment) -> str:
    """Return the fixed GitHub API endpoint for the current repository."""
    repository = _environment_value(environment, "GITHUB_REPOSITORY")
    owner, separator, name = repository.partition("/")
    if not separator:
        raise WorkflowEnvironmentError("GITHUB_REPOSITORY")
    if not owner or not name:
        raise WorkflowEnvironmentError("GITHUB_REPOSITORY")
    return f"https://api.github.com/repos/{owner}/{name}/check-runs"


def _publish_check_run(environment: Environment, payload: dict[str, object]) -> None:
    """Publish a bounded Check Run through GitHub's REST API."""
    token = _environment_value(environment, "GITHUB_TOKEN")
    request = urllib.request.Request(  # ruff:ignore[suspicious-url-open-usage] -- fixed GitHub API host.
        _check_run_endpoint(environment),
        data=json.dumps(payload).encode("utf-8"),
        headers={
            "Accept": "application/vnd.github+json",
            "Authorization": f"Bearer {token}",
            "X-GitHub-Api-Version": GITHUB_API_VERSION,
        },
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=30) as response:  # ruff:ignore[suspicious-url-open-usage] -- fixed request.
        response.read()


def report_coverage(environment: Environment) -> None:
    """Publish the same-repository coverage Check Run and its conclusion output."""
    values = _report_values(environment)
    conclusion = coverage_conclusion(
        values["download_outcome"],
        values["validation_outcome"],
        values["submission_outcome"],
    )
    _publish_check_run(
        environment,
        _check_run_payload(
            environment, conclusion, _report_summary(values, conclusion)
        ),
    )
    _write_output(environment, "conclusion", conclusion)


def report_excluded_fork(environment: Environment) -> None:
    """Publish the neutral required Check Run for an excluded fork PR."""
    _publish_check_run(
        environment,
        _check_run_payload(environment, "neutral", _fork_summary(environment)),
    )


def summarize_coverage(environment: Environment) -> None:
    """Append bounded trusted coverage correlation fields to the job summary."""
    lines = ["### CodeScene coverage", ""]
    lines.extend(
        f"{label}: {_environment_value(environment, environment_name)}"
        for label, environment_name in SUMMARY_FIELDS
    )
    lines.append(f"- Conclusion: {_environment_value(environment, 'CONCLUSION')}")
    _append_workflow_value(environment, "GITHUB_STEP_SUMMARY", "\n".join(lines))


def _arguments(argv: cabc.Sequence[str] | None = None) -> argparse.Namespace:
    """Parse one fixed trusted workflow action command."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "command",
        choices=(
            "start-telemetry",
            "record-telemetry",
            "validate-artefact",
            "report-coverage",
            "report-excluded-fork",
            "summarize-coverage",
        ),
    )
    parser.add_argument("--artifact-directory")
    return parser.parse_args(argv)


def main(argv: cabc.Sequence[str] | None = None) -> int:
    """Run one trusted coverage action command.

    Returns
    -------
    int
        ``0`` after a completed action or the hostile-artefact validator's
        documented exit status.

    Raises
    ------
    WorkflowEnvironmentError
        If the selected action is missing a required bounded argument.
    """
    arguments = _arguments(argv)
    environment = os.environ
    match arguments.command:
        case "start-telemetry":
            start_telemetry(environment)
            return 0
        case "record-telemetry":
            record_telemetry(environment)
            return 0
        case "validate-artefact":
            artifact_directory = arguments.artifact_directory
            if artifact_directory is None:
                raise WorkflowEnvironmentError("artifact-directory")
            return validate_artefact(artifact_directory)
        case "report-coverage":
            report_coverage(environment)
            return 0
        case "report-excluded-fork":
            report_excluded_fork(environment)
            return 0
        case "summarize-coverage":
            summarize_coverage(environment)
            return 0
    raise WorkflowEnvironmentError(arguments.command)


if __name__ == "__main__":
    raise SystemExit(main())
