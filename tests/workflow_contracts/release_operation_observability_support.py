"""Hold the tables and workflow readers the release-observability contracts use.

Each release job is asserted twice over: once for the steps its telemetry
summary reads, and once for the bounded record those steps produce. Both
readings start from the same two tables — which phases an operation has, and
which step ids carry a phase's timing markers — so those tables live here
rather than being restated by the module that asserts with them.

Nothing here holds a test of its own. The claims live in
:mod:`release_operation_observability_test`; this module is the fixture layer
that keeps both files inside the repository's 400-line cap, following
:mod:`release_publication_wiring` and its sibling contracts.

Run via ``make test-workflow-contracts``.
"""

from release_operation_observability import OperationObservation, StepState
from workflow_loading import named_step, require_mapping, step_index_by_key

#: The phases a staged release job runs. Staging and publication share these
#: four; packaging builds one crate and hands it on, so it has none of them.
STAGED_PHASES = (
    "draft_release",
    "artifact_download",
    "archive_hoist",
    "upload_plan_validation",
)
#: The phases the packaging job runs, which are its own two boundaries.
PACKAGING_PHASES = ("crate_packaging", "artifact_upload")
#: The step-id prefix each phase's timing markers carry. Draft creation has no
#: markers: its outcome is the step's own, so there is nothing to time around
#: it. The upload plan's markers are named for the plan rather than for the
#: validation phase, which is what the workflow has always called them.
MARKER_PREFIXES = {
    "draft_release": (),
    "artifact_download": ("artifact_download",),
    "archive_hoist": ("archive_hoist",),
    "upload_plan_validation": ("upload_plan",),
    "crate_packaging": ("crate_packaging",),
    "artifact_upload": ("artifact_upload",),
}
OBSERVABILITY_SUMMARY_STEP = "Record release operation observability"
OBSERVABILITY_ARTIFACT_STEP = "Upload release operation observability"
DRY_RUN_INPUT = "${{ needs.metadata.outputs.should_publish != 'true' }}"


def timed_phases(phases: tuple[str, ...]) -> tuple[str, ...]:
    """Return the phases among ``phases`` that carry start/end timing markers.

    Draft creation is reported through its step's own outcome, so it has no
    marked start and end. Filtering by :data:`MARKER_PREFIXES` keeps that one
    exception stated in the table rather than repeated at each assertion.

    Parameters
    ----------
    phases
        Phase labels to filter.

    Returns
    -------
    tuple[str, ...]
        The subset whose markers exist in the workflow.
    """
    return tuple(phase for phase in phases if MARKER_PREFIXES[phase])


def assert_staged_steps_expose_outcomes(steps: list[dict[str, object]]) -> None:
    """Require timing anchors and staged phases to expose their outcomes."""
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


def assert_packaging_steps_expose_outcomes(steps: list[dict[str, object]]) -> None:
    """Require the packaging job's own two phases to expose their outcomes.

    The package step and the artifact upload are the job's boundaries: one
    compiles and digests the crate, the other hands it to the next job. A phase
    whose outcome is unreadable cannot be reported, so both carry an ``id``.
    """
    named_step(steps, "Start release operation timing")
    for step_id, label in (
        ("package_crate", "crate packaging"),
        ("upload_crate_package", "artifact upload"),
    ):
        step = steps[step_index_by_key(steps, "id", step_id)]
        assert step.get("id"), f"{label} must expose a step outcome"


def assert_summary_environment(
    steps: list[dict[str, object]],
    operation: str,
    dry_run: str,
    *,
    staged: bool,
) -> dict[str, object]:
    """Check fixed summary labels and reject raw upload error data.

    Parameters
    ----------
    steps
        The job's parsed steps.
    operation
        The fixed operation label the summary must carry.
    dry_run
        The expression or literal the summary's dry-run input must hold.
    staged
        Whether the job validates an upload plan, which decides where the
        upload-error flag comes from.

    Returns
    -------
    dict[str, object]
        The summary step's environment, for the phase assertions to read.
    """
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
    expected_upload_error = (
        "${{ steps.upload_assets.outputs.upload-error == 'true' }}"
        if staged
        else "false"
    )
    assert env.get("UPLOAD_ERROR_PRESENT") == expected_upload_error, (
        "the summary must record only whether the action reports an upload error"
    )
    assert "ERROR_MESSAGE" not in env, "the summary must not receive raw error text"
    assert "error-message" not in env, "the summary must not receive raw error text"
    assert "python3 scripts/release_operation_observability.py" in str(
        summary.get("run", "")
    ), "the summary helper must run without depending on a prior setup step"
    return env


def assert_summary_phase_timings(
    env: dict[str, object], phases: tuple[str, ...]
) -> None:
    """Require outcomes and start/end times for every phase the job runs."""
    for phase in phases:
        assert env.get(f"{phase.upper()}_OUTCOME"), (
            f"the summary must record the {phase} outcome"
        )
        assert env.get(f"{phase.upper()}_STARTED_NS"), (
            f"the summary must record the {phase} start time"
        )
        assert env.get(f"{phase.upper()}_ENDED_NS"), (
            f"the summary must record the {phase} end time"
        )


def assert_draft_outcome_policy(env: dict[str, object], *, creates_draft: bool) -> None:
    """Record draft creation where the job creates a draft, and only there.

    Packaging does not create a draft, so a ``DRAFT_RELEASE_OUTCOME`` there
    would describe a phase the job never runs. Publication does, and its
    outcome is the only evidence the summary has that the phase ran at all.
    """
    if creates_draft:
        assert env.get("DRAFT_RELEASE_OUTCOME") == (
            "${{ steps.draft_release.outcome }}"
        ), "publication telemetry must record draft creation"
    else:
        assert "DRAFT_RELEASE_OUTCOME" not in env, (
            "packaging telemetry must not model draft creation"
        )


def assert_staging_telemetry_policy(
    steps: list[dict[str, object]], env: dict[str, object]
) -> None:
    """Keep draft data and diagnostic artifact uploads out of dry-run staging."""
    assert "DRAFT_RELEASE_OUTCOME" not in env, (
        "staging telemetry must not model draft creation"
    )
    assert not any(step.get("name") == OBSERVABILITY_ARTIFACT_STEP for step in steps), (
        "dry-run staging must not upload a diagnostic telemetry artifact"
    )


def assert_retained_telemetry_policy(steps: list[dict[str, object]]) -> None:
    """Require a job whose telemetry must outlive the runner to upload it.

    Publication retains the record because an operator inspects it after the
    release is live, and packaging retains it for the same reason: the job that
    consumes the archive is a different job, and the run's own log is not where
    a later failure is diagnosed from.
    """
    artifact = named_step(steps, OBSERVABILITY_ARTIFACT_STEP)
    assert artifact.get("if") == "always()", "telemetry upload must run after failures"
    assert str(artifact.get("uses", "")).startswith("actions/upload-artifact@"), (
        "telemetry must be retained as an Actions artifact"
    )
    artifact_with = require_mapping(
        artifact.get("with"), "observability artifact inputs"
    )
    assert artifact_with.get("name") == (
        "release-operation-observability-${{ github.job }}"
    ), "telemetry must use a job-specific artifact name"
    assert artifact_with.get("path") == (
        "${{ runner.temp }}/release-operation-observability.jsonl"
    ), "the uploaded artifact must contain the bounded JSONL records"
    assert "retention-days" not in artifact_with, (
        "telemetry must use the repository's default artifact retention"
    )
    assert step_index_by_key(
        steps, "name", OBSERVABILITY_SUMMARY_STEP
    ) < step_index_by_key(steps, "name", OBSERVABILITY_ARTIFACT_STEP), (
        "the summary file must be written before telemetry is uploaded"
    )


def assert_timing_markers_run_after_failures(
    steps: list[dict[str, object]], phases: tuple[str, ...]
) -> None:
    """Require every phase's timing markers to run after a preceding failure.

    A marker guarded only on success is skipped exactly when its phase fails,
    which is the case the telemetry exists to explain. The marker names come
    from :data:`MARKER_PREFIXES` rather than from the phase label, so a phase
    whose markers are named for what is being timed keeps its coverage.
    """
    for phase in phases:
        for prefix in MARKER_PREFIXES[phase]:
            for marker_id in (f"{prefix}_start", f"{prefix}_end"):
                marker = steps[step_index_by_key(steps, "id", marker_id)]
                assert marker.get("if") == "always()", (
                    f"{marker_id} must run after a failed preceding phase"
                )


def staging_observation() -> OperationObservation:
    """Create a representative successful dry-run staging observation.

    Returns
    -------
    OperationObservation
        An observation whose executed phases all succeeded and whose draft
        phase was skipped, which is what a dry-run staging passes to the
        renderer.
    """
    steps = {
        phase: StepState(
            "skipped" if phase == "draft_release" else "success",
            1_000_000_000,
            2_000_000_000,
        )
        for phase in STAGED_PHASES
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


__all__ = [
    "DRY_RUN_INPUT",
    "MARKER_PREFIXES",
    "OBSERVABILITY_ARTIFACT_STEP",
    "OBSERVABILITY_SUMMARY_STEP",
    "PACKAGING_PHASES",
    "STAGED_PHASES",
    "assert_draft_outcome_policy",
    "assert_packaging_steps_expose_outcomes",
    "assert_retained_telemetry_policy",
    "assert_staged_steps_expose_outcomes",
    "assert_staging_telemetry_policy",
    "assert_summary_environment",
    "assert_summary_phase_timings",
    "assert_timing_markers_run_after_failures",
    "staging_observation",
    "timed_phases",
]
