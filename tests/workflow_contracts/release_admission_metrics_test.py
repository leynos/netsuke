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
    workflow = load_workflow(RELEASE_WORKFLOW_PATH)
    admission = workflow_job(workflow, "release-admission-canaries")
    release = workflow_job(workflow, "release")
    release_needs = release.get("needs")
    permissions = require_mapping(
        admission.get("permissions"), "release-admission-canaries.permissions"
    )
    steps = job_steps(workflow, "release-admission-canaries")
    admission_step = next(
        step
        for step in steps
        if step.get("name") == "Require release-admission evidence"
    )
    summary_step = next(
        step
        for step in steps
        if step.get("name") == "Summarise release-admission metrics"
    )
    upload_step = next(
        step for step in steps if step.get("name") == "Upload release-admission metrics"
    )

    assert permissions == ADMISSION_PERMISSIONS, (
        "admission must retain read-only permissions"
    )
    assert admission.get("continue-on-error") is True, (
        "the scaffold must not block releases before evidence production exists"
    )
    assert (
        require_mapping(admission_step.get("env"), "release admission step environment")
        == ADMISSION_ENVIRONMENT
    ), "admission must scope its GitHub token and runner-local metrics file"
    assert "require-release-admission-canaries.sh" in str(admission_step.get("run")), (
        "admission must execute the instrumented gate"
    )
    assert step_index_by_key(
        steps, "name", "Require release-admission evidence"
    ) < step_index_by_key(steps, "name", "Upload release-admission metrics"), (
        "the gate must write metrics before their artifact upload"
    )
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
    assert isinstance(release_needs, list), "release dependencies must be a list"
    assert "release-admission-canaries" not in release_needs, (
        "publication must not depend on a scaffold without an evidence producer"
    )
    assert "release-admission-canaries" not in str(release.get("if")), (
        "publication must defer evidence gating until the producer is available"
    )
