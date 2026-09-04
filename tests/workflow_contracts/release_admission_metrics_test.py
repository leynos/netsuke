"""Pin bounded release-admission metric delivery in the RFC 0005 scaffold."""

from workflow_loading import (
    RELEASE_WORKFLOW_PATH,
    job_steps,
    load_workflow,
    require_mapping,
    step_index_by_key,
    workflow_job,
)

ADMISSION_PERMISSIONS = {"actions": "read", "contents": "read"}
METRICS_FILE_ENV = {
    "NETSUKE_RELEASE_ADMISSION_METRICS_FILE": (
        "${{ runner.temp }}/release-admission-metrics.jsonl"
    ),
}
ADMISSION_ENVIRONMENT = {
    "GH_TOKEN": "${{ secrets.GITHUB_TOKEN }}",
    "NETSUKE_RELEASE_ADMISSION_ENFORCE": "false",
    **METRICS_FILE_ENV,
}
METRICS_ARTIFACT = {
    "name": "release-admission-metrics",
    "path": "${{ env.NETSUKE_RELEASE_ADMISSION_METRICS_FILE }}",
    "if-no-files-found": "error",
}
UPLOAD_CONDITION = (
    "always() && needs.metadata.outputs.should_upload_workflow_artifacts == 'true'"
)
SUMMARY_REQUIRED_FRAGMENTS = frozenset({
    '>>"$GITHUB_STEP_SUMMARY"',
    "steps.release_admission.outputs.gate-outcome",
    "steps.release_admission.outputs.gate-error-category",
})


def test_release_admission_metrics_retain_read_only_delivery() -> None:
    """Verify the scaffold retains bounded metrics on every non-dry-run outcome.

    Notes
    -----
    The scaffold remains read-only and non-blocking until a real evidence
    producer exists. Its summary always runs, and its artifact upload preserves
    failure diagnostics while respecting the reusable dry-run contract.
    """
    admission, release, admission_step, summary_step, upload_step = _workflow_parts()
    _assert_admission_job_contract(admission, admission_step)
    _assert_metrics_delivery_contract(summary_step, upload_step)
    _assert_release_scaffold_boundary(release)


def _workflow_parts() -> tuple[dict[str, object], ...]:
    """Return the workflow mappings exercised by the delivery contract."""
    workflow = load_workflow(RELEASE_WORKFLOW_PATH)
    steps = job_steps(workflow, "release-admission-canaries")
    assert step_index_by_key(
        steps, "name", "Require release-admission evidence"
    ) < step_index_by_key(steps, "name", "Upload release-admission metrics"), (
        "the gate must write metrics before their artifact upload"
    )
    return (
        workflow_job(workflow, "release-admission-canaries"),
        workflow_job(workflow, "release"),
        _step_named(steps, "Require release-admission evidence"),
        _step_named(steps, "Summarise release-admission metrics"),
        _step_named(steps, "Upload release-admission metrics"),
    )


def _step_named(steps: list[dict[str, object]], name: str) -> dict[str, object]:
    """Return the required workflow step with the supplied fixed name."""
    return next(step for step in steps if step.get("name") == name)


def _assert_admission_job_contract(
    admission: dict[str, object], admission_step: dict[str, object]
) -> None:
    """Assert the admission job retains its read-only non-blocking boundary."""
    permissions = require_mapping(
        admission.get("permissions"), "release-admission-canaries.permissions"
    )
    assert permissions == ADMISSION_PERMISSIONS, (
        "admission must retain read-only permissions"
    )
    assert "continue-on-error" not in admission, (
        "observation mode must succeed without suppressing other admission failures"
    )
    assert (
        require_mapping(admission_step.get("env"), "release admission step environment")
        == ADMISSION_ENVIRONMENT
    ), "admission must scope its GitHub token and runner-local metrics file"
    assert "require-release-admission-canaries.sh" in str(admission_step.get("run")), (
        "admission must execute the instrumented gate"
    )


def _assert_metrics_delivery_contract(
    summary_step: dict[str, object], upload_step: dict[str, object]
) -> None:
    """Assert the summary and artefact preserve failure-path diagnostics."""
    assert summary_step.get("if") == "always()", (
        "the summary must run after both successful and failed admission checks"
    )
    summary_run = str(summary_step.get("run"))
    assert all(fragment in summary_run for fragment in SUMMARY_REQUIRED_FRAGMENTS), (
        "the summary must write its operator-facing output and both gate fields"
    )
    assert "actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a" in str(
        upload_step.get("uses")
    ), "the metrics artifact upload must stay SHA-pinned"
    assert upload_step.get("with") == METRICS_ARTIFACT, (
        "the artifact must retain the bounded metrics file"
    )
    assert upload_step.get("env") == METRICS_FILE_ENV, (
        "the upload must resolve the same runner-local metrics file"
    )
    assert upload_step.get("if") == UPLOAD_CONDITION, (
        "the upload must retain failures while respecting the dry-run contract"
    )


def _assert_release_scaffold_boundary(release: dict[str, object]) -> None:
    """Assert publication defers enforcement until evidence production exists."""
    release_needs = release.get("needs")
    assert isinstance(release_needs, list), "release dependencies must be a list"
    assert "release-admission-canaries" not in release_needs, (
        "publication must not depend on a scaffold without an evidence producer"
    )
    assert "release-admission-canaries" not in str(release.get("if")), (
        "publication must defer evidence gating until the producer is available"
    )
