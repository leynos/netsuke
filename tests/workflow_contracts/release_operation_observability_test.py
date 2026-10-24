"""Contracts for bounded observability on the release workflow's jobs.

Each release path writes an always-run summary and fixed-label local JSONL
metrics and phase events. Staging writes the summary only; publication and
packaging also upload the JSONL, because both hand an outcome to a later job or
to an operator after the runner is gone. The helper tests below pin the record
shape and ensure no free-form release data enters telemetry.

The wiring tables and step assertions live in
:mod:`release_operation_observability_support` so both files stay inside the
repository's 400-line cap.

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
from release_operation_observability_support import (
    DRY_RUN_INPUT,
    PACKAGING_PHASES,
    STAGED_PHASES,
    assert_draft_outcome_policy,
    assert_packaging_steps_expose_outcomes,
    assert_retained_telemetry_policy,
    assert_staged_steps_expose_outcomes,
    assert_staging_telemetry_policy,
    assert_summary_environment,
    assert_summary_phase_timings,
    assert_timing_markers_run_after_failures,
    staging_observation,
    timed_phases,
)
from workflow_loading import RELEASE_WORKFLOW_PATH, job_steps, load_workflow


@pytest.fixture(scope="module")
def release_workflow() -> dict[str, object]:
    """Load the release workflow once for the shared wiring checks."""
    return load_workflow(RELEASE_WORKFLOW_PATH)


@pytest.mark.parametrize(
    ("job_name", "operation", "dry_run"),
    [
        ("release", "release_staging", "true"),
        ("publish-release", "release_publication", DRY_RUN_INPUT),
        ("package-crate", "release_package", DRY_RUN_INPUT),
    ],
)
def test_release_jobs_emit_always_run_bounded_observability(
    release_workflow: dict[str, object], job_name: str, operation: str, dry_run: str
) -> None:
    """Every release path must retain fixed-label outcomes and durations."""
    steps = job_steps(release_workflow, job_name)
    staged = job_name != "package-crate"
    phases = STAGED_PHASES if staged else PACKAGING_PHASES

    if staged:
        assert_staged_steps_expose_outcomes(steps)
    else:
        assert_packaging_steps_expose_outcomes(steps)

    env = assert_summary_environment(steps, operation, dry_run, staged=staged)
    # Draft creation carries no timing markers — its outcome is the step's own —
    # so it is asserted through the per-job policy below rather than here.
    assert_summary_phase_timings(env, timed_phases(phases))

    if job_name == "release":
        assert_staging_telemetry_policy(steps, env)
    else:
        assert_draft_outcome_policy(env, creates_draft=job_name == "publish-release")
        assert_retained_telemetry_policy(steps)

    assert_timing_markers_run_after_failures(steps, phases)


def test_observability_records_fixed_metrics_without_unbounded_data() -> None:
    """Render bounded operation and phase metrics for staging."""
    summary, jsonl = render_observation(staging_observation())
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
    expected_phases = set(STAGED_PHASES) - {"draft_release"}
    assert {
        record["phase"] for record in metrics if "phase" in record
    } == expected_phases, "phase metrics must use only fixed release phase labels"


def test_observability_records_bounded_phase_events() -> None:
    """Render phase start and completion events without unbounded data."""
    _, jsonl = render_observation(staging_observation())
    records = [json.loads(line) for line in jsonl.splitlines()]
    traces = [record for record in records if record["record_type"] == "trace_event"]
    expected_phases = set(STAGED_PHASES) - {"draft_release"}

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
    steps = {phase: StepState("success", None, None) for phase in STAGED_PHASES}
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
    steps = {phase: StepState("skipped", None, None) for phase in STAGED_PHASES}
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
        steps={phase: StepState("skipped", None, None) for phase in STAGED_PHASES},
        upload_error_present=False,
    )

    with pytest.raises(ValueError, match="unsupported release operation"):
        render_observation(observation)


def test_observability_reports_only_the_phases_of_its_operation() -> None:
    """Require each operation to report its own phases and no other's.

    Shared phase labels across operations would make packaging report four
    phases it never runs, each permanently ``skipped``. A phase that can never
    fail reads as one that always passes, and a reader of the summary is given
    no way to tell that from a phase the job simply does not have.
    """
    packaging = OperationObservation(
        operation="release_package",
        dry_run=True,
        job_status="success",
        started_ns=1_000_000_000,
        ended_ns=2_000_000_000,
        steps={
            "crate_packaging": StepState("success", 1_000_000_000, 1_500_000_000),
            "artifact_upload": StepState("success", 1_500_000_000, 2_000_000_000),
        },
        upload_error_present=False,
    )

    summary, jsonl = render_observation(packaging)
    records = [json.loads(line) for line in jsonl.splitlines()]

    assert {record.get("phase") for record in records if "phase" in record} == set(
        PACKAGING_PHASES
    ), "packaging telemetry must report only the phases packaging runs"
    phase_lines = [line for line in summary.splitlines() if "`crate_packaging`" in line]
    assert len(phase_lines) == 1, "the summary must report crate packaging exactly once"
    assert not any(f"`{phase}`" in summary for phase in STAGED_PHASES), (
        "packaging telemetry must not report a release phase it never runs"
    )


def test_observability_reads_packaging_values_from_the_environment() -> None:
    """Parse the packaging job's own fixed environment into an observation."""
    environment = {
        "RELEASE_OPERATION": "release_package",
        "RELEASE_DRY_RUN": "false",
        "JOB_STATUS": "failure",
        "OPERATION_STARTED_NS": "1000000000",
        "CRATE_PACKAGING_OUTCOME": "failure",
        "CRATE_PACKAGING_STARTED_NS": "1100000000",
        "CRATE_PACKAGING_ENDED_NS": "1200000000",
        "ARTIFACT_UPLOAD_OUTCOME": "skipped",
        "ARTIFACT_UPLOAD_STARTED_NS": "",
        "ARTIFACT_UPLOAD_ENDED_NS": "",
        "UPLOAD_ERROR_PRESENT": "false",
    }

    observation = observation_from_environment(environment)
    summary, _ = render_observation(observation)

    assert set(observation.steps) == set(PACKAGING_PHASES), (
        "the environment must be read for exactly the packaging phases"
    )
    assert observation.steps["crate_packaging"].outcome == "failure", (
        "a failed package step must be recorded as the crate packaging outcome"
    )
    assert "Error category: `crate_packaging`" in summary, (
        "a failed packaging step must map to the fixed crate-packaging category"
    )
