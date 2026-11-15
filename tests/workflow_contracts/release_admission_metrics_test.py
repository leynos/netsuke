"""Pin bounded release-admission metric delivery in the RFC 0005 scaffold."""

from action_references import require_external_action_sha
from workflow_loading import (
    RELEASE_WORKFLOW_PATH,
    REPO_ROOT,
    job_steps,
    load_workflow,
    require_mapping,
    step_index_by_key,
    workflow_job,
)

ADMISSION_PERMISSIONS = {"actions": "read", "contents": "read"}
ADMISSION_TIMEOUT_MINUTES = 15
#: The action that provisions the interpreter the admission timings run under.
ADMISSION_PYTHON_ACTION = "astral-sh/setup-uv"
#: The action that retrieves the evidence a producer uploaded for this run.
ADMISSION_ARTIFACT_ACTION = "actions/download-artifact"
#: The action that retains the admission metrics and traces as artefacts.
ARTIFACT_UPLOAD_ACTION = "actions/upload-artifact"
EVIDENCE_ARTIFACT = "release-admission-evidence"
EVIDENCE_DIRECTORY = "${{ runner.temp }}/release-admission-evidence"
METRICS_FILE_ENV = {
    "NETSUKE_RELEASE_ADMISSION_METRICS_FILE": (
        "${{ runner.temp }}/release-admission-metrics.jsonl"
    ),
}
TRACE_FILE_ENV = {
    "NETSUKE_RELEASE_ADMISSION_TRACE_FILE": (
        "${{ runner.temp }}/release-admission-traces.jsonl"
    ),
}
#: Enforcement is the release mode, not a separate setting: a publishing run
#: must fail closed and a rehearsal must keep observing.
ADMISSION_ENVIRONMENT = {
    "GH_TOKEN": "${{ secrets.GITHUB_TOKEN }}",
    "NETSUKE_RELEASE_ADMISSION_ENFORCE": (
        "${{ needs.metadata.outputs.should_publish == 'true' }}"
    ),
    "NETSUKE_RELEASE_ADMISSION_TAG": "${{ needs.metadata.outputs.release_tag }}",
    "NETSUKE_RELEASE_ADMISSION_EVIDENCE_FILE": f"{EVIDENCE_DIRECTORY}/evidence.json",
    **METRICS_FILE_ENV,
    **TRACE_FILE_ENV,
}
#: The mode, outcome and candidate every write-authority job compares before it
#: writes.
ADMISSION_OUTPUTS = {
    "gate_outcome": "${{ steps.release_admission.outputs.gate-outcome }}",
    "enforcement": "${{ steps.release_admission.outputs.enforcement }}",
    "candidate_commit": "${{ steps.release_admission.outputs.candidate-commit }}",
}
METRICS_ARTIFACT = {
    "name": "release-admission-metrics",
    "path": "${{ env.NETSUKE_RELEASE_ADMISSION_METRICS_FILE }}",
    "if-no-files-found": "error",
}
TRACE_ARTIFACT = {
    "name": "release-admission-traces",
    "path": "${{ env.NETSUKE_RELEASE_ADMISSION_TRACE_FILE }}",
    "if-no-files-found": "error",
}
UPLOAD_CONDITION = (
    "always() && needs.metadata.outputs.should_upload_workflow_artifacts == 'true'"
)
SUMMARY_ENVIRONMENT = {
    "RELEASE_ADMISSION_GATE_OUTCOME": (
        "${{ steps.release_admission.outputs.gate-outcome }}"
    ),
    "RELEASE_ADMISSION_ERROR_CATEGORY": (
        "${{ steps.release_admission.outputs.gate-error-category }}"
    ),
}
SUMMARY_REQUIRED_FRAGMENTS = frozenset({
    '>>"$GITHUB_STEP_SUMMARY"',
    "\\`$RELEASE_ADMISSION_GATE_OUTCOME\\`",
    "\\`$RELEASE_ADMISSION_ERROR_CATEGORY\\`",
})
#: The estate rule is that gate logic is Python: the admission step runs the
#: PEP 723 entry point through ``uv``, which is what resolves its dependencies,
#: rather than executing a shell script from the repository.
ADMISSION_ENTRY_POINT = ".github/scripts/release_admission.py"
ADMISSION_COMMAND = f"uv run --script {ADMISSION_ENTRY_POINT}"
#: The shell gate this entry point replaced. A residual copy would be dead
#: code, and a residual reference would mean the workflow still ran it.
RETIRED_ADMISSION_SCRIPTS = (
    "require-release-admission-canaries.sh",
    "release-admission-adapters.sh",
    "release-admission-policy.sh",
)


def test_release_admission_metrics_retain_read_only_delivery() -> None:
    """Verify the gate keeps bounded diagnostics while failing closed.

    Notes
    -----
    The job holds read-only permissions and publishes its mode, outcome and
    candidate as outputs. Its summary always runs, and its artifact uploads
    preserve failure diagnostics while respecting the reusable dry-run
    contract.

    Notes
    -----
    The contract covers read-only permissions, evidence retrieval, the derived
    enforcement value, the rehearsal job's independence, and metric and trace
    retention on applicable outcomes.
    """
    (
        admission,
        release,
        python_step,
        evidence_step,
        admission_step,
        summary_step,
        upload_step,
        trace_upload,
    ) = _workflow_parts()
    _assert_admission_job_contract(
        admission, python_step, evidence_step, admission_step
    )
    _assert_metrics_delivery_contract(summary_step, upload_step, trace_upload)
    _assert_release_scaffold_boundary(release)


def _workflow_parts() -> tuple[dict[str, object], ...]:
    """Return the workflow mappings exercised by the delivery contract.

    Returns
    -------
    tuple[dict[str, object], ...]
        The workflow, release job, Python setup step, admission step, summary
        step, metric upload step, and trace upload step, in that order.

    """
    workflow = load_workflow(RELEASE_WORKFLOW_PATH)
    steps = job_steps(workflow, "release-admission-canaries")
    assert step_index_by_key(
        steps, "name", "Require release-admission evidence"
    ) < step_index_by_key(steps, "name", "Upload release-admission metrics"), (
        "the gate must write metrics before their artifact upload"
    )
    assert step_index_by_key(
        steps, "name", "Upload release-admission metrics"
    ) < step_index_by_key(steps, "name", "Upload release-admission traces"), (
        "the trace delivery must follow trace production and metric delivery"
    )
    assert step_index_by_key(
        steps, "name", "Install Python for admission timings"
    ) < step_index_by_key(steps, "name", "Require release-admission evidence"), (
        "the admission job must install Python before collecting durations"
    )
    assert step_index_by_key(
        steps, "name", "Download the release-admission evidence"
    ) < step_index_by_key(steps, "name", "Require release-admission evidence"), (
        "the gate must retrieve the evidence before it reads it"
    )
    return (
        workflow_job(workflow, "release-admission-canaries"),
        workflow_job(workflow, "release"),
        _step_named(steps, "Install Python for admission timings"),
        _step_named(steps, "Download the release-admission evidence"),
        _step_named(steps, "Require release-admission evidence"),
        _step_named(steps, "Summarise release-admission metrics"),
        _step_named(steps, "Upload release-admission metrics"),
        _step_named(steps, "Upload release-admission traces"),
    )


def _step_named(steps: list[dict[str, object]], name: str) -> dict[str, object]:
    """Return the required workflow step with the supplied fixed name.

    Parameters
    ----------
    steps
        Workflow steps in declaration order.
    name
        Exact fixed step name to find.

    Returns
    -------
    dict[str, object]
        The first step whose ``name`` field matches ``name``.

    """
    return next(step for step in steps if step.get("name") == name)


def _assert_admission_job_contract(
    admission: dict[str, object],
    python_step: dict[str, object],
    evidence_step: dict[str, object],
    admission_step: dict[str, object],
) -> None:
    """Assert the admission job retains its read-only, candidate-bound contract.

    Parameters
    ----------
    admission
        Parsed release-admission job mapping.
    python_step
        Python setup step used for monotonic duration collection.
    evidence_step
        Step that retrieves the producer's evidence artefact.
    admission_step
        Gate execution step whose environment and command are inspected.

    """
    permissions = require_mapping(
        admission.get("permissions"), "release-admission-canaries.permissions"
    )
    assert permissions == ADMISSION_PERMISSIONS, (
        "admission must retain read-only permissions"
    )
    assert admission.get("outputs") == ADMISSION_OUTPUTS, (
        "admission must expose its mode, outcome and candidate to every writer"
    )
    require_external_action_sha(
        evidence_step.get("uses"),
        ADMISSION_ARTIFACT_ACTION,
        "the admission job's evidence retrieval",
    )
    assert evidence_step.get("with") == {
        "name": EVIDENCE_ARTIFACT,
        "path": EVIDENCE_DIRECTORY,
    }, "the gate must read the evidence uploaded for this run and no other"
    assert evidence_step.get("continue-on-error") is True, (
        "absent evidence must reach the verifier as missing evidence rather "
        "than as a download failure the gate cannot classify"
    )
    assert admission.get("timeout-minutes") == ADMISSION_TIMEOUT_MINUTES, (
        "admission must bound its overall runtime"
    )
    assert "continue-on-error" not in admission, (
        "observation mode must succeed without suppressing other admission failures"
    )
    require_external_action_sha(
        python_step.get("uses"),
        ADMISSION_PYTHON_ACTION,
        "the admission job's Python provisioning for monotonic duration collection",
    )
    assert python_step.get("with") == {
        "python-version": "3.14",
        "enable-cache": False,
    }, "the admission Python installation must use the pinned baseline without a cache"
    assert (
        require_mapping(admission_step.get("env"), "release admission step environment")
        == ADMISSION_ENVIRONMENT
    ), "admission must scope its GitHub token and runner-local metrics file"
    assert admission_step.get("run") == ADMISSION_COMMAND, (
        "admission must run the Python entry point, whose PEP 723 block uv resolves"
    )


def test_admission_gate_is_python_and_its_shell_predecessors_are_gone() -> None:
    """Require the gate to be Python, with no shell predecessor left behind.

    Notes
    -----
    The estate rule under ``docs/scripting-standards.md`` puts gate logic in a
    ``uv``-run Python script, so this asserts both halves: the Python entry
    point the workflow runs exists, and none of the three shell scripts it
    replaced survives anywhere the workflow could still reach.
    """
    admission = REPO_ROOT / ADMISSION_ENTRY_POINT
    assert admission.is_file(), (
        f"the admission step must have {ADMISSION_ENTRY_POINT} to run"
    )
    scripts = REPO_ROOT / ".github" / "scripts"
    survivors = [
        name for name in RETIRED_ADMISSION_SCRIPTS if (scripts / name).exists()
    ]
    assert not survivors, (
        f"the retired admission shell scripts must be deleted: {survivors}"
    )
    for workflow in sorted((REPO_ROOT / ".github" / "workflows").glob("*.yml")):
        referenced = [
            name
            for name in RETIRED_ADMISSION_SCRIPTS
            if name in workflow.read_text("utf-8")
        ]
        assert not referenced, (
            f"{workflow.name} must not reference the retired admission scripts "
            f"{referenced}; admission is the Python entry point now"
        )


def _assert_metrics_delivery_contract(
    summary_step: dict[str, object],
    upload_step: dict[str, object],
    trace_upload: dict[str, object],
) -> None:
    """Assert summary and artefact steps preserve failure diagnostics.

    Parameters
    ----------
    summary_step
        Job-summary step that reports the gate outcome.
    upload_step
        Metrics artefact upload step.
    trace_upload
        Trace artefact upload step.

    """
    assert summary_step.get("if") == "always()", (
        "the summary must run after both successful and failed admission checks"
    )
    summary_run = str(summary_step.get("run"))
    assert all(fragment in summary_run for fragment in SUMMARY_REQUIRED_FRAGMENTS), (
        "the summary must write its operator-facing output and both gate fields"
    )
    assert summary_step.get("env") == SUMMARY_ENVIRONMENT, (
        "the summary must pass both action outputs through the step environment"
    )
    require_external_action_sha(
        upload_step.get("uses"),
        ARTIFACT_UPLOAD_ACTION,
        "the metrics artifact upload",
    )
    assert upload_step.get("with") == METRICS_ARTIFACT, (
        "the artifact must retain the bounded metrics file"
    )
    assert upload_step.get("env") == METRICS_FILE_ENV, (
        "the upload must resolve the same runner-local metrics file"
    )
    assert upload_step.get("if") == UPLOAD_CONDITION, (
        "the upload must retain failures while respecting the dry-run contract"
    )
    assert trace_upload.get("if") == UPLOAD_CONDITION, (
        "the trace upload must retain the metrics outcome-retention policy"
    )
    assert trace_upload.get("with") == TRACE_ARTIFACT, (
        "the trace artifact must retain the bounded trace file"
    )
    assert trace_upload.get("env") == TRACE_FILE_ENV, (
        "the trace upload must resolve the same runner-local trace file"
    )


def _assert_release_scaffold_boundary(release: dict[str, object]) -> None:
    """Assert the read-only rehearsal lane stays independent of the gate.

    Parameters
    ----------
    release
        Parsed *staging* job mapping, not the publication job.

    Notes
    -----
    A rehearsal must remain reachable when packaging or admission fails, so the
    staging job neither depends on the gate nor tests its outputs. The gate's
    own enforcement is what keeps that safe: a rehearsal observes, and the
    write-authority jobs -- never this one -- require enforcement.
    """
    release_needs = release.get("needs")
    assert isinstance(release_needs, list), "release dependencies must be a list"
    assert "release-admission-canaries" not in release_needs, (
        "the rehearsal lane must stay reachable when admission fails"
    )
    assert "release-admission-canaries" not in str(release.get("if")), (
        "the rehearsal lane must not be gated on admission evidence"
    )
