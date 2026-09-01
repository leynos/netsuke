"""Pin bounded release-admission metric delivery before release publication."""

from workflow_loading import (
    RELEASE_WORKFLOW_PATH,
    job_steps,
    load_workflow,
    require_mapping,
    step_index_by_key,
    workflow_job,
)

ADMISSION_PERMISSIONS = {"actions": "read", "contents": "read"}
ADMISSION_TOKEN = {"GH_TOKEN": "${{ secrets.GITHUB_TOKEN }}"}
METRICS_ARTIFACT = {
    "name": "release-admission-metrics",
    "path": "${{ env.NETSUKE_RELEASE_ADMISSION_METRICS_FILE }}",
    "if-no-files-found": "error",
}


def test_release_admission_metrics_retain_read_only_delivery() -> None:
    """The admission job must emit, summarise, and retain metrics before release."""
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
    admission_environment = require_mapping(
        admission_step.get("env"), "release admission step environment"
    )

    assert permissions == ADMISSION_PERMISSIONS, (
        "admission must retain read-only permissions"
    )
    assert admission_environment == ADMISSION_TOKEN, (
        "admission must scope its GitHub token"
    )
    assert "NETSUKE_RELEASE_ADMISSION_METRICS_FILE" in require_mapping(
        admission.get("env"), "release admission environment"
    ), "admission must provide its metrics-file path"
    assert "require-release-admission-canaries.sh" in str(admission_step.get("run")), (
        "admission must execute the instrumented gate"
    )
    assert step_index_by_key(
        steps, "name", "Require release-admission evidence"
    ) < step_index_by_key(steps, "name", "Upload release-admission metrics"), (
        "the gate must write metrics before their artifact upload"
    )
    upload_step = next(
        step for step in steps if step.get("name") == "Upload release-admission metrics"
    )
    assert "actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a" in str(
        upload_step.get("uses")
    ), "the metrics artifact upload must stay SHA-pinned"
    assert upload_step.get("with") == METRICS_ARTIFACT, (
        "the artifact must retain the bounded metrics file"
    )
    assert isinstance(release_needs, list), "release dependencies must be a list"
    assert "release-admission-canaries" in release_needs, (
        "publication must depend on release admission"
    )
    assert "needs.release-admission-canaries.result == 'success'" in str(
        release.get("if")
    ), "publication must require a successful release admission result"
