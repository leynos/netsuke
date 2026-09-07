"""Exercise the pure Python outcome seam used by trusted coverage reporting.

The GitHub workflow invokes this checked-in Python module after hostile-data
validation. Loading it directly keeps the outcome decision executable without
workflow event data or network access.
"""

import functools
import importlib.util
import json
import sys
import typing as typ

import pytest
from workflow_loading import (
    COVERAGE_PR_WORKFLOW_PATH,
    REPO_ROOT,
    job_steps,
    load_workflow,
    named_step,
)

if typ.TYPE_CHECKING:
    import pathlib
    import types

ACTION_MODULE_PATH = REPO_ROOT / ".github" / "scripts" / "coverage_pr_submission.py"
OBSERVABILITY_MODULE_PATH = (
    REPO_ROOT / ".github" / "scripts" / "coverage_pr_submission_observability.py"
)
UPLOAD_ARTIFACT_ACTION = (
    "actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a"
)
OBSERVABILITY_UPLOAD_CONDITION = (
    "always() && steps.observe_download_coverage.outcome == 'success'"
)


@functools.cache
def _coverage_action_module() -> types.ModuleType:
    """Load the checked-in trusted coverage Python action module."""
    specification = importlib.util.spec_from_file_location(
        "coverage_pr_submission", ACTION_MODULE_PATH
    )
    assert specification is not None, "the trusted action module must be loadable"
    assert specification.loader is not None, (
        "the trusted action module must provide a loader"
    )
    module = importlib.util.module_from_spec(specification)
    sys.modules[specification.name] = module
    specification.loader.exec_module(module)
    return module


@functools.cache
def _observability_action_module() -> types.ModuleType:
    """Load the checked-in trusted coverage observability Python module."""
    specification = importlib.util.spec_from_file_location(
        "coverage_pr_submission_observability", OBSERVABILITY_MODULE_PATH
    )
    assert specification is not None, (
        "the trusted observability module must be loadable"
    )
    assert specification.loader is not None, (
        "the trusted observability module must provide a loader"
    )
    module = importlib.util.module_from_spec(specification)
    sys.modules[specification.name] = module
    specification.loader.exec_module(module)
    return module


@pytest.mark.parametrize(
    ("download_outcome", "validation_outcome", "submission_outcome", "expected"),
    [
        pytest.param("success", "success", "success", "success", id="success"),
        pytest.param("success", "success", "skipped", "neutral", id="absent-token"),
        pytest.param("failure", "skipped", "skipped", "failure", id="download-failure"),
        pytest.param(
            "success", "failure", "skipped", "failure", id="validation-failure"
        ),
        pytest.param(
            "success", "success", "failure", "failure", id="submission-failure"
        ),
    ],
)
def test_coverage_conclusion_preserves_stage_outcomes(
    download_outcome: str,
    validation_outcome: str,
    submission_outcome: str,
    expected: str,
) -> None:
    """Map every trusted-handoff terminal state to its Check Run conclusion."""
    action_module = _coverage_action_module()
    assert (
        action_module.coverage_conclusion(
            download_outcome, validation_outcome, submission_outcome
        )
        == expected
    ), "the Check Run conclusion must match its three stage outcomes"


def test_coverage_observability_writes_bounded_metrics_and_traces(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Export stage metrics and traces without unbounded PR-controlled fields."""
    module = _observability_action_module()
    metrics_path = tmp_path / "metrics.jsonl"
    traces_path = tmp_path / "traces.jsonl"
    output_path = tmp_path / "output"
    environment = {
        "GITHUB_OUTPUT": str(output_path),
        "STARTED_AT_MS": "1000",
        "OPERATION": "codescene-submission",
        "OUTCOME": "failure",
        "ORIGINATING_WORKFLOW_RUN_ID": "123",
        "ORIGINATING_COMMIT_SHA": "a" * 40,
        module.METRICS_FILE_ENVIRONMENT_KEY: str(metrics_path),
        module.TRACES_FILE_ENVIRONMENT_KEY: str(traces_path),
    }

    module.record_telemetry(environment, now_milliseconds=1250)

    metrics = [json.loads(line) for line in metrics_path.read_text().splitlines()]
    traces = [json.loads(line) for line in traces_path.read_text().splitlines()]
    expected_metrics = [
        {
            "name": module.OPERATION_METRIC_NAME,
            "labels": {
                "operation": "codescene-submission",
                "outcome": "failure",
                "error_category": "stage_failure",
            },
            "value": 1,
        },
        {
            "name": module.DURATION_METRIC_NAME,
            "labels": {"operation": "codescene-submission"},
            "value": 0.25,
        },
    ]
    expected_traces = [
        {
            "event": module.TRACE_EVENT,
            "operation": "codescene-submission",
            "outcome": "failure",
            "error_category": "stage_failure",
            "duration_seconds": 0.25,
            "workflow_run_id": "123",
            "commit_sha": "a" * 40,
        }
    ]
    assert metrics == expected_metrics, (
        "metrics must retain their fixed names, labels, and duration"
    )
    assert traces == expected_traces, (
        "traces must retain only bounded stage and correlation fields"
    )
    assert output_path.read_text() == "duration_ms=250\n", (
        "recorded observability must retain the workflow duration output"
    )
    assert len(capsys.readouterr().out.splitlines()) == 3, (
        "metrics and traces must be emitted to the workflow log"
    )


@pytest.mark.parametrize(
    ("step_name", "artifact_name", "environment_name", "file_name"),
    [
        pytest.param(
            "Upload CodeScene coverage metrics",
            "codescene-pr-coverage-metrics",
            "NETSUKE_CODESCENE_COVERAGE_METRICS_FILE",
            "codescene-pr-coverage-metrics.jsonl",
            id="metrics",
        ),
        pytest.param(
            "Upload CodeScene coverage traces",
            "codescene-pr-coverage-traces",
            "NETSUKE_CODESCENE_COVERAGE_TRACES_FILE",
            "codescene-pr-coverage-traces.jsonl",
            id="traces",
        ),
    ],
)
def test_workflow_uploads_bounded_coverage_observability(
    step_name: str,
    artifact_name: str,
    environment_name: str,
    file_name: str,
) -> None:
    """Retain each trusted observability artefact after a telemetry record."""
    workflow = load_workflow(COVERAGE_PR_WORKFLOW_PATH)
    step = named_step(job_steps(workflow, "submit-coverage"), step_name)

    assert step.get("if") == OBSERVABILITY_UPLOAD_CONDITION, (
        "observability uploads must follow successful telemetry initialisation"
    )
    assert step.get("uses") == UPLOAD_ARTIFACT_ACTION, (
        "observability uploads must retain their SHA-pinned action"
    )
    assert step.get("env") == {
        environment_name: f"${{{{ runner.temp }}}}/{file_name}"
    }, "observability uploads must retain only their runner-local output path"
    expected_inputs = {
        "name": artifact_name,
        "path": f"${{{{ env.{environment_name} }}}}",
        "if-no-files-found": "error",
    }
    assert step.get("with") == expected_inputs, (
        "observability uploads must preserve their fixed bounded artefact contract"
    )
