"""Hold Kani metrics marks, finalization and artifact delivery to contract."""

import json
import textwrap

from action_references import require_external_action_sha
from kani_smoke_scope_wiring_test import PROOF_CONDITION, PROOF_END_CONDITION
from workflow_loading import (
    job_steps,
    load_workflow,
    named_step,
    require_mapping,
)

JOB = "kani-smoke"
STATE_DIRECTORY = "${{ runner.temp }}/kani-smoke-metrics"
DECISION_FILE = f"{STATE_DIRECTORY}/decision.json"
METRICS_FILE = f"{STATE_DIRECTORY}/metrics.jsonl"
TIMING_FAILURE_WARNING = "::warning::Kani proof timing mark could not be recorded."
FINALIZER_ENV = {
    "INPUT_EVENT_NAME": "${{ github.event_name }}",
    "INPUT_JOB_STATUS": "${{ job.status }}",
    "INPUT_SCOPE_OUTCOME": "${{ steps.scope.outcome }}",
    "INPUT_RUN_PROOFS": "${{ steps.scope.outputs.run-proofs }}",
    "INPUT_CACHE_OUTCOME": "${{ steps.kani_cache.outcome }}",
    "INPUT_CACHE_HIT": "${{ steps.kani_cache.outputs.cache-hit }}",
    "INPUT_PROOFS_OUTCOME": "${{ steps.proofs.outcome }}",
    "INPUT_MUTATIONS_OUTCOME": "${{ steps.mutations.outcome }}",
    "INPUT_END_MARK_RECORDED": "${{ steps.proof_end.outputs.recorded }}",
    "INPUT_END_MARK_OUTCOME": "${{ steps.proof_end.outcome }}",
    "INPUT_STATE_DIRECTORY": STATE_DIRECTORY,
    "INPUT_DECISION_FILE": DECISION_FILE,
    "INPUT_METRICS_FILE": METRICS_FILE,
}


def _workflow_steps() -> list[dict[str, object]]:
    """Return the Kani job's steps in workflow order."""
    return job_steps(load_workflow(), JOB)


def _env(step: dict[str, object], description: str) -> dict[str, object]:
    """Return one step's environment mapping with a useful failure context."""
    return require_mapping(step.get("env"), f"{description} env")


def test_proof_marks_bracket_the_exact_guarded_make_target() -> None:
    """Place both fixed mark commands directly around the proof execution."""
    steps = _workflow_steps()
    start = named_step(steps, "Mark proof start")
    proof = named_step(steps, "Run Kani harnesses")
    end = named_step(steps, "Mark proof end")
    assert steps.index(start) + 1 == steps.index(proof), (
        "the proof start mark must be the step directly before make kani-ir"
    )
    assert steps.index(proof) + 1 == steps.index(end), (
        "the proof end mark must be the step directly after make kani-ir"
    )
    assert start.get("if") == PROOF_CONDITION, (
        "the proof start timing mark must use the exact run-proofs guard"
    )
    assert end.get("if") == "${{ " + PROOF_END_CONDITION + " }}", (
        "the end mark must retain its guard after a proof failure"
    )
    assert start.get("run") == (
        "uv run --script scripts/kani_smoke_metrics.py mark-start 2>/dev/null || "
        f"printf '%s\\n' '{TIMING_FAILURE_WARNING}'"
    ), "start-mark failures must warn while allowing proof work to continue"
    assert end.get("id") == "proof_end", (
        "the finalizer must distinguish a failed mark from a skipped step"
    )
    expected_end_run = textwrap.dedent(
        """\
        if uv run --script scripts/kani_smoke_metrics.py mark-end 2>/dev/null; then
          printf '%s\\n' 'recorded=true' >> "$GITHUB_OUTPUT"
        else
          printf '%s\\n' 'recorded=false' >> "$GITHUB_OUTPUT"
          printf '%s\\n' '::warning::Kani proof timing mark could not be recorded.'
        fi
        """
    )
    assert end.get("run") == expected_end_run, (
        "end-mark failure must be recorded without stopping later proof gates"
    )
    for step in (start, end):
        assert "continue-on-error" not in step, (
            "timing failures must be non-fatal through the fixed warning command"
        )
    for step, name in ((start, "start"), (end, "end")):
        assert _env(step, f"Mark proof {name}") == {
            "INPUT_STATE_DIRECTORY": STATE_DIRECTORY
        }, f"the {name} mark must receive only the shared state directory"
    assert proof.get("id") == "proofs", "the finalizer reads the proof step outcome"
    mutations = named_step(steps, "Mutation patch compile gate")
    assert mutations.get("id") == "mutations", (
        "the finalizer reads the mutation step outcome"
    )


def test_finalizer_runs_after_save_and_receives_only_bounded_inputs() -> None:
    """Pass every classification input through environment, after cache save."""
    steps = _workflow_steps()
    finalizer = named_step(steps, "Finalize Kani metrics")
    save = named_step(steps, "Save Kani payloads")
    assert steps.index(save) < steps.index(finalizer), (
        "the finalizer must observe the cache-save step's terminal state"
    )
    assert finalizer.get("if") == "always()", (
        "the finalizer must run after ordinary failures and explicit skips"
    )
    assert (
        finalizer.get("run") == "uv run --script scripts/kani_smoke_metrics.py finalize"
    ), "the finalizer must use the shared metrics helper"
    assert _env(finalizer, "Finalize Kani metrics") == FINALIZER_ENV, (
        "the finalizer must receive bounded event, outcome and path inputs"
    )
    forbidden_keys = {"continue-on-error", "uses", "permissions"}
    assert not forbidden_keys.intersection(finalizer), (
        "the finalizer must not weaken failure handling or add permissions"
    )
    command = str(finalizer.get("run", ""))
    assert all(token not in command for token in ("make ", "cargo ", "kani ")), (
        "the finalizer must not run proof or build commands"
    )
    assert "secrets." not in json.dumps(finalizer), (
        "the finalizer must not receive secrets"
    )


def test_artifact_upload_is_last_retained_and_path_matched() -> None:
    """Retain one per-attempt artifact and make missing emission fail visibly."""
    steps = _workflow_steps()
    finalizer = named_step(steps, "Finalize Kani metrics")
    upload = named_step(steps, "Upload Kani metrics")
    assert steps[-1] is upload, "metrics upload must remain the final Kani job step"
    assert upload.get("if") == "always()", (
        "artifact delivery must run after job failures"
    )
    require_external_action_sha(
        upload.get("uses"),
        "actions/upload-artifact",
        "Kani metrics artifact upload",
    )
    assert upload.get("uses") == (
        "actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a"
    ), "Kani metrics must use the requested upload-artifact revision"
    upload_inputs = require_mapping(upload.get("with"), "Kani metrics upload inputs")
    assert len(upload_inputs) == 4, "the upload must have exactly four inputs"
    assert (
        upload_inputs.get("name") == "kani-smoke-metrics-${{ github.run_attempt }}"
    ), "the upload name must include the run attempt"
    assert upload_inputs.get("path") == METRICS_FILE, (
        "the upload must use the finalizer's metrics path"
    )
    assert upload_inputs.get("retention-days") == 30, (
        "the upload must retain metrics for 30 days"
    )
    assert upload_inputs.get("if-no-files-found") == "error", (
        "a missing metrics file must fail delivery visibly"
    )
    finalizer_env = _env(finalizer, "Finalize Kani metrics")
    assert finalizer_env["INPUT_METRICS_FILE"] == upload_inputs["path"], (
        "the finalizer output and artifact path must match exactly"
    )
    assert "secrets." not in json.dumps(upload), (
        "artifact delivery must not receive secrets"
    )
    assert not {"continue-on-error", "permissions"}.intersection(upload), (
        "artifact upload must retain its normal failure and permission policy"
    )
    assert "run" not in upload, "artifact delivery must not run shell commands"


def test_decision_record_path_is_shared_with_finalizer() -> None:
    """Keep scope decision emission and finalizer consumption on one file."""
    steps = _workflow_steps()
    scope = named_step(steps, "Decide Kani proof scope")
    assert _env(scope, "Decide Kani proof scope") == {
        "INPUT_EVENT_NAME": "${{ github.event_name }}",
        "INPUT_METRICS_FILE": DECISION_FILE,
    }, "scope must receive the separate fixed decision-record location"
    finalizer_env = _env(
        named_step(steps, "Finalize Kani metrics"), "Finalize Kani metrics"
    )
    assert finalizer_env["INPUT_DECISION_FILE"] == DECISION_FILE, (
        "the finalizer must read the decision file written by the scope step"
    )
