"""Contracts for bounded observability on release staging and publication.

Both release paths write an always-run summary and fixed-label local JSONL
metrics and phase events; only publication uploads the JSONL. The helper tests
below pin the record shape and ensure no free-form release data enters telemetry.

Run via ``make test-workflow-contracts``.
"""

import json

import pytest
from release_operation_observability import (
    OperationObservation,
    StepState,
    observation_from_environment,
    render_observation,
)
from workflow_loading import (
    RELEASE_WORKFLOW_PATH,
    job_steps,
    load_workflow,
    named_step,
    require_mapping,
    step_index_by_key,
)

PHASES = (
    "draft_release",
    "artifact_download",
    "archive_hoist",
    "upload_plan_validation",
)
OBSERVABILITY_SUMMARY_STEP = "Record release operation observability"
OBSERVABILITY_ARTIFACT_STEP = "Upload release operation observability"


@pytest.fixture(scope="module")
def release_workflow() -> dict[str, object]:
    """Load the release workflow once for the shared wiring checks."""
    return load_workflow(RELEASE_WORKFLOW_PATH)


@pytest.mark.parametrize(
    ("job_name", "operation", "dry_run"),
    [
        ("release", "release_staging", "true"),
        (
            "publish-release",
            "release_publication",
            "${{ needs.metadata.outputs.should_publish != 'true' }}",
        ),
    ],
)
def test_release_jobs_emit_always_run_bounded_observability(
    release_workflow: dict[str, object], job_name: str, operation: str, dry_run: str
) -> None:
    """Both release paths must retain fixed-label outcomes and durations."""
    steps = job_steps(release_workflow, job_name)
    named_step(steps, "Start release operation timing")

    download = steps[step_index_by_key(steps, "id", "download_artifacts")]
    hoist = steps[step_index_by_key(steps, "id", "archive_hoist")]
    upload = steps[step_index_by_key(steps, "id", "upload_assets")]
    for step, label in (
        (download, "artifact download"),
        (hoist, "archive hoist"),
        (upload, "upload plan"),
    ):
        assert step.get("id"), f"{label} must expose a step outcome"

    summary = named_step(steps, OBSERVABILITY_SUMMARY_STEP)
    assert summary.get("if") == "always()", (
        "release observability must run after a failed operation"
    )
    env = require_mapping(summary.get("env"), "release observability environment")
    assert env.get("RELEASE_OPERATION") == operation, (
        "the summary must identify the fixed release operation"
    )
    assert env.get("RELEASE_DRY_RUN") == dry_run, (
        "the summary must record the operation's dry-run mode"
    )
    assert env.get("UPLOAD_ERROR_PRESENT") == (
        "${{ steps.upload_assets.outputs.upload-error == 'true' }}"
    ), "the summary must record only whether the action reports an upload error"
    assert "ERROR_MESSAGE" not in env, "the summary must not receive raw error text"
    assert "error-message" not in env, "the summary must not receive raw error text"
    assert "python3 scripts/release_operation_observability.py" in str(
        summary.get("run", "")
    ), "the summary helper must run without depending on a prior setup step"
    for phase in ("artifact_download", "archive_hoist", "upload_plan_validation"):
        assert env.get(f"{phase.upper()}_OUTCOME"), (
            f"the summary must record the {phase} outcome"
        )
        assert env.get(f"{phase.upper()}_STARTED_NS"), (
            f"the summary must record the {phase} start time"
        )
        assert env.get(f"{phase.upper()}_ENDED_NS"), (
            f"the summary must record the {phase} end time"
        )

    if job_name == "release":
        assert "DRAFT_RELEASE_OUTCOME" not in env, (
            "staging telemetry must not model draft creation"
        )
        assert not any(
            step.get("name") == OBSERVABILITY_ARTIFACT_STEP for step in steps
        ), "dry-run staging must not upload a diagnostic telemetry artifact"
    else:
        assert (
            env.get("DRAFT_RELEASE_OUTCOME") == "${{ steps.draft_release.outcome }}"
        ), "publication telemetry must record draft creation"
        artifact = named_step(steps, OBSERVABILITY_ARTIFACT_STEP)
        assert artifact.get("if") == "always()", (
            "publication telemetry upload must run after failures"
        )
        assert str(artifact.get("uses", "")).startswith("actions/upload-artifact@"), (
            "publication telemetry must be retained as an Actions artifact"
        )
        artifact_with = require_mapping(
            artifact.get("with"), "observability artifact inputs"
        )
        assert artifact_with.get("name") == (
            "release-operation-observability-${{ github.job }}"
        ), "publication telemetry must use a job-specific artifact name"
        assert artifact_with.get("path") == (
            "${{ runner.temp }}/release-operation-observability.jsonl"
        ), "the uploaded artifact must contain the bounded JSONL records"
        assert "retention-days" not in artifact_with, (
            "telemetry must use the repository's default artifact retention"
        )
        assert step_index_by_key(
            steps, "name", OBSERVABILITY_SUMMARY_STEP
        ) < step_index_by_key(steps, "name", OBSERVABILITY_ARTIFACT_STEP), (
            "the summary file must be written before publication telemetry is uploaded"
        )

    for marker_id in (
        "artifact_download_start",
        "artifact_download_end",
        "archive_hoist_start",
        "archive_hoist_end",
        "upload_plan_start",
        "upload_plan_end",
    ):
        marker = steps[step_index_by_key(steps, "id", marker_id)]
        assert marker.get("if") == "always()", (
            f"{marker_id} must run after a failed preceding phase"
        )


def _staging_observation() -> OperationObservation:
    """Create a representative successful dry-run staging observation."""
    steps = {
        phase: StepState(
            "skipped" if phase == "draft_release" else "success",
            1_000_000_000,
            2_000_000_000,
        )
        for phase in PHASES
    }
    return OperationObservation(
        operation="release_staging",
        dry_run=True,
        job_status="success",
        started_ns=1_000_000_000,
        ended_ns=5_000_000_000,
        steps=steps,
        upload_error_present=False,
    )


def test_observability_records_fixed_metrics_without_unbounded_data() -> None:
    """Render bounded operation and phase metrics for staging."""
    observation = _staging_observation()

    summary, jsonl = render_observation(observation)
    records = [json.loads(line) for line in jsonl.splitlines()]
    metrics = [record for record in records if record["record_type"] == "metric"]

    assert summary.splitlines()[2:7] == [
        "- Operation: `release_staging`",
        "- Dry run: `true`",
        "- Job status: `success`",
        "- Error category: `none`",
        "- Upload error output present: `false`",
    ], "the step summary must report bounded operation status"
    assert any(record["metric"] == "release_operation_total" for record in metrics), (
        "telemetry must count release operations"
    )
    assert any(
        record["metric"] == "release_operation_duration_seconds" for record in metrics
    ), "telemetry must record the release operation duration"
    assert (
        sum(record["metric"] == "release_phase_duration_seconds" for record in metrics)
        == 3
    ), "telemetry must record durations for each executed phase"
    expected_phases = set(PHASES) - {"draft_release"}
    assert {
        record["phase"] for record in metrics if "phase" in record
    } == expected_phases, "phase metrics must use only fixed release phase labels"


def test_observability_records_bounded_phase_events() -> None:
    """Render phase start and completion events without unbounded data."""
    _, jsonl = render_observation(_staging_observation())
    records = [json.loads(line) for line in jsonl.splitlines()]
    traces = [record for record in records if record["record_type"] == "trace_event"]
    expected_phases = set(PHASES) - {"draft_release"}

    assert {record["event"] for record in traces} == {
        "release_phase_started",
        "release_phase_completed",
    }, "successful phases must have start and completion events"
    assert {record["phase"] for record in traces} == expected_phases, (
        "phase events must use only fixed release phase labels"
    )
    assert "release-tag" not in jsonl, "telemetry must not include release tags"
    assert "raw error" not in jsonl, "telemetry must not include raw action errors"


def test_observability_maps_upload_action_errors_to_a_fixed_category() -> None:
    """Record a failing upload plan without copying its error payload."""
    steps = {phase: StepState("success", None, None) for phase in PHASES}
    observation = OperationObservation(
        operation="release_publication",
        dry_run=False,
        job_status="failure",
        started_ns=None,
        ended_ns=3_000_000_000,
        steps=steps,
        upload_error_present=True,
    )

    summary, jsonl = render_observation(observation)

    assert "Error category: `upload_plan_validation`" in summary, (
        "an upload action error must use the fixed upload-plan category"
    )
    assert '"error_category":"upload_plan_validation"' in jsonl, (
        "the JSONL records must retain the fixed upload-plan category"
    )
    assert "raw error" not in summary + jsonl, (
        "summaries and telemetry must not expose raw action errors"
    )


def test_observability_reads_fixed_step_values_from_the_environment() -> None:
    """Parse the named workflow outcomes and timestamps without raw errors."""
    environment = {
        "RELEASE_OPERATION": "release_staging",
        "RELEASE_DRY_RUN": "true",
        "JOB_STATUS": "success",
        "OPERATION_STARTED_NS": "1000000000",
        "ARTIFACT_DOWNLOAD_OUTCOME": "success",
        "ARTIFACT_DOWNLOAD_STARTED_NS": "1100000000",
        "ARTIFACT_DOWNLOAD_ENDED_NS": "1200000000",
        "ARCHIVE_HOIST_OUTCOME": "success",
        "ARCHIVE_HOIST_STARTED_NS": "1300000000",
        "ARCHIVE_HOIST_ENDED_NS": "1400000000",
        "UPLOAD_PLAN_VALIDATION_OUTCOME": "success",
        "UPLOAD_PLAN_VALIDATION_STARTED_NS": "1500000000",
        "UPLOAD_PLAN_VALIDATION_ENDED_NS": "1600000000",
        "UPLOAD_ERROR_PRESENT": "false",
    }

    observation = observation_from_environment(environment)

    assert observation.operation == "release_staging", (
        "the environment operation label must be preserved"
    )
    assert observation.dry_run is True, "the dry-run flag must be parsed"
    assert observation.steps["draft_release"].outcome == "skipped", (
        "staging must not create a draft release"
    )
    assert observation.steps["artifact_download"].started_ns == 1_100_000_000, (
        "the artifact download start time must be parsed"
    )
    assert observation.steps["upload_plan_validation"].ended_ns == 1_600_000_000, (
        "the upload plan end time must be parsed"
    )
    assert observation.upload_error_present is False, (
        "the environment must not claim an upload error"
    )


def test_observability_closes_a_cancelled_phase_with_a_bounded_event() -> None:
    """Record an interrupted phase without losing the cancellation outcome."""
    steps = {phase: StepState("skipped", None, None) for phase in PHASES}
    steps["artifact_download"] = StepState("cancelled", 1_000_000_000, None)
    observation = OperationObservation(
        operation="release_staging",
        dry_run=True,
        job_status="cancelled",
        started_ns=1_000_000_000,
        ended_ns=2_000_000_000,
        steps=steps,
        upload_error_present=False,
    )

    _, jsonl = render_observation(observation)
    records = [json.loads(line) for line in jsonl.splitlines()]
    events = [record for record in records if record["record_type"] == "trace_event"]

    assert any(record.get("outcome") == "cancelled" for record in records), (
        "cancellation must remain visible in telemetry"
    )
    assert {record["event"] for record in events} == {
        "release_phase_started",
        "release_phase_interrupted",
    }, "an interrupted phase must have start and interruption events"
    assert all(record["phase"] == "artifact_download" for record in events), (
        "interruption events must identify the cancelled phase"
    )


def test_observability_rejects_unknown_operation_names() -> None:
    """Reject operation labels outside the fixed low-cardinality set."""
    observation = OperationObservation(
        operation="release-for-tag-abc123",
        dry_run=False,
        job_status="success",
        started_ns=None,
        ended_ns=1,
        steps={phase: StepState("skipped", None, None) for phase in PHASES},
        upload_error_present=False,
    )

    with pytest.raises(ValueError, match="unsupported release operation"):
        render_observation(observation)
