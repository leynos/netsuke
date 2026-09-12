"""Run trusted CodeScene PR coverage workflow actions with Python only.

The trusted workflow invokes this module through GitHub Actions' ``python``
shell. It accepts fixed commands, emits bounded correlation data, and never
reads pull-request content or artefact members as executable code.
"""

import argparse
import collections.abc as cabc
import importlib.util
import os
import pathlib
import sys
import time
import typing as typ

if typ.TYPE_CHECKING:
    import types

type Environment = cabc.Mapping[str, str]
type ValidatorMain = cabc.Callable[[cabc.Sequence[str] | None], int]
type CheckRunPublisher = cabc.Callable[[dict[str, object]], None]

REPOSITORY_ROOT = pathlib.Path(__file__).resolve().parents[2]
ARCHIVE_VALIDATOR_PATH = REPOSITORY_ROOT / "scripts" / "validate_coverage_archive.py"
CHECK_RUN_PUBLISHER_PATH = pathlib.Path(__file__).with_name("coverage_pr_check_runs.py")
REPORTING_PATH = pathlib.Path(__file__).with_name("coverage_pr_reporting.py")


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
    """Record one trusted workflow stage's start time.

    Parameters
    ----------
    environment
        GitHub Actions environment containing ``GITHUB_OUTPUT``.
    now_milliseconds
        Optional clock seam for deterministic tests. ``None`` uses the current
        Unix timestamp in milliseconds.

    Notes
    -----
    Writes the bounded ``started_at_ms`` value to GitHub's step-output file.
    """
    started_at_ms = (
        _now_milliseconds() if now_milliseconds is None else now_milliseconds
    )
    _write_output(environment, "started_at_ms", str(started_at_ms))


def _archive_validator_main() -> ValidatorMain:
    """Load the trusted hostile-archive validator through its explicit seam."""
    spec = importlib.util.spec_from_file_location(
        "validate_coverage_archive", ARCHIVE_VALIDATOR_PATH
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
    """Validate and safely materialize a downloaded archive as hostile data.

    Parameters
    ----------
    artifact_directory
        Directory containing the raw archive retained by the download action.

    Returns
    -------
    int
        The hostile-artefact validator's documented process exit status.

    Notes
    -----
    The validator inspects the archive before extraction and writes only the
    validated ``lcov.info`` member to ``validated-coverage``.
    """
    return _archive_validator_main()([
        "--archive-dir",
        artifact_directory,
        "--output-dir",
        "validated-coverage",
    ])


def _reporting_module() -> types.ModuleType:
    """Load the pure trusted reporting builder through its explicit seam."""
    specification = importlib.util.spec_from_file_location(
        "coverage_pr_reporting", REPORTING_PATH
    )
    if specification is None or specification.loader is None:
        raise ValidatorLoadError
    module = importlib.util.module_from_spec(specification)
    sys.modules[specification.name] = module
    specification.loader.exec_module(module)
    return module


def _github_check_run_publisher(environment: Environment) -> CheckRunPublisher:
    """Build the real Check Run publisher for the trusted GitHub API boundary."""
    specification = importlib.util.spec_from_file_location(
        "coverage_pr_check_runs", CHECK_RUN_PUBLISHER_PATH
    )
    if specification is None or specification.loader is None:
        raise ValidatorLoadError
    module = importlib.util.module_from_spec(specification)
    sys.modules[specification.name] = module
    specification.loader.exec_module(module)
    transport = module.GitHubApiTransport(
        _environment_value(environment, "GITHUB_TOKEN")
    )
    publisher = module.GitHubCheckRunPublisher(
        _environment_value(environment, "GITHUB_REPOSITORY"), transport
    )
    return typ.cast("CheckRunPublisher", publisher.publish)


def report_coverage(environment: Environment, publisher: CheckRunPublisher) -> None:
    """Publish the same-repository coverage Check Run and conclusion output.

    Parameters
    ----------
    environment
        Bounded stage outcomes, correlation values, and the GitHub output path.
    publisher
        Check Run publisher selected at the command boundary.

    Notes
    -----
    Publishes the completed Check Run and appends its conclusion to
    ``GITHUB_OUTPUT`` for later summary and telemetry stages.
    """
    reporting = _reporting_module()
    values = reporting.report_values(lambda name: _environment_value(environment, name))
    conclusion = coverage_conclusion(
        values["download_outcome"],
        values["validation_outcome"],
        values["submission_outcome"],
    )
    publisher(
        reporting.check_run_payload(
            lambda name: _environment_value(environment, name),
            conclusion,
            reporting.summary(values, reporting.REPORT_FIELDS, conclusion),
        ),
    )
    _write_output(environment, "conclusion", conclusion)


def report_excluded_fork(
    environment: Environment, publisher: CheckRunPublisher
) -> None:
    """Publish the neutral required Check Run for an excluded fork PR.

    Parameters
    ----------
    environment
        Bounded originating workflow values and GitHub repository context.
    publisher
        Check Run publisher selected at the command boundary.

    Notes
    -----
    Publishes a neutral result without downloading, validating, or executing
    fork-controlled artefact data.
    """
    reporting = _reporting_module()
    publisher(
        reporting.check_run_payload(
            lambda name: _environment_value(environment, name),
            "neutral",
            reporting.fork_summary(lambda name: _environment_value(environment, name)),
        )
    )


def summarize_coverage(environment: Environment) -> None:
    """Append bounded trusted coverage correlation fields to the job summary.

    Parameters
    ----------
    environment
        Bounded workflow correlation values, stage outcomes, durations, and
        the ``GITHUB_STEP_SUMMARY`` path.

    Notes
    -----
    Appends fixed labels and GitHub-supplied values to the workflow summary;
    it does not read coverage data or pull-request metadata.
    """
    summary = _reporting_module().workflow_summary(
        lambda name: _environment_value(environment, name)
    )
    _append_workflow_value(environment, "GITHUB_STEP_SUMMARY", summary)


def _arguments(argv: cabc.Sequence[str] | None = None) -> argparse.Namespace:
    """Parse one fixed trusted workflow action command."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "command",
        choices=(
            "start-telemetry",
            "validate-artefact",
            "report-coverage",
            "report-excluded-fork",
            "summarize-coverage",
        ),
    )
    parser.add_argument("--artifact-directory")
    return parser.parse_args(argv)


def main(
    argv: cabc.Sequence[str] | None = None,
    environment: Environment | None = None,
    publisher: CheckRunPublisher | None = None,
) -> int:
    """Run one trusted coverage action command.

    Parameters
    ----------
    argv
        Command arguments. ``None`` uses the process arguments.
    environment
        GitHub Actions environment. ``None`` uses :data:`os.environ`.
    publisher
        Optional Check Run publisher seam. ``None`` constructs the real,
        fixed-GitHub publisher only for reporting commands.

    Returns
    -------
    int
        ``0`` after a completed action or the hostile-artefact validator's
        documented exit status.

    Raises
    ------
    WorkflowEnvironmentError
        If the selected action is missing a required bounded argument.

    Notes
    -----
    Dispatches only fixed commands. Reporting side effects cross the injected
    publisher boundary; validation delegates only to the checked-in validator.
    """
    arguments = _arguments(argv)
    selected_environment = os.environ if environment is None else environment
    match arguments.command:
        case "start-telemetry":
            start_telemetry(selected_environment)
            return 0
        case "validate-artefact":
            artifact_directory = arguments.artifact_directory
            if artifact_directory is None:
                raise WorkflowEnvironmentError("artifact-directory")
            return validate_artefact(artifact_directory)
        case "report-coverage":
            report_coverage(
                selected_environment,
                publisher or _github_check_run_publisher(selected_environment),
            )
            return 0
        case "report-excluded-fork":
            report_excluded_fork(
                selected_environment,
                publisher or _github_check_run_publisher(selected_environment),
            )
            return 0
        case "summarize-coverage":
            summarize_coverage(selected_environment)
            return 0
    raise WorkflowEnvironmentError(arguments.command)


if __name__ == "__main__":
    raise SystemExit(main())
