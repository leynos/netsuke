"""Observe the upload step's own log and timing record.

The upload step refuses by deferring: it writes an ``upload-error`` output and
exits zero, so the caller's own step is what fails the job and what normally
prints the diagnostic. That step is not guaranteed to run -- a runner that
cannot retain the plan artefact, or a cancellation between the two, skips it --
so the step that refused must also say why in its own log.

The boundary timings are the other half. A single phase duration for the whole
upload step cannot tell a slow transfer from a listing that retried, and the
plan beside it says nothing about how long anything took. These cases hold the
record to what the run actually crossed, including the boundaries it did not.

Run via ``make test-workflow-contracts``.
"""

import dataclasses
import json
import typing as typ

from upload_declared_assets_harness import ActionRun, Release, execute
from upload_declared_assets_manifest import RUN_STEPS
from upload_declared_assets_staging import (
    recorded_digests,
    release_facts,
    stage_workspace,
)

if typ.TYPE_CHECKING:
    from pathlib import Path

#: The step that refuses, and so the step whose log must explain the refusal.
UPLOAD_STEP_INDEX = RUN_STEPS.index(RUN_STEPS[-1])

#: What the step writes before deferring the failure to its caller.
REFUSAL_PREFIX = "declared asset upload refused:"

#: The bounded field set one completed trace record may carry. A completed call
#: carries no ``error_category``, so its presence here would be a writer that
#: attributed a failure to a call that returned.
TRACE_FIELDS = frozenset({
    "record_type",
    "event",
    "operation",
    "timestamp_ns",
    "outcome",
    "duration_seconds",
    "run_id",
    "job",
    "run_attempt",
})


@dataclasses.dataclass(frozen=True, slots=True)
class RefusedRun:
    """Hold a refused run and the asset whose bytes it disagreed over.

    Attributes
    ----------
    run
        The completed steps, the outputs, and the recorded ``gh`` calls.
    offender
        The declared asset the stub release attached with wrong bytes.
    """

    run: ActionRun
    offender: str

    def timings(self) -> dict[str, object]:
        """Return the boundary timings the upload step retained.

        Returns
        -------
        dict[str, object]
            The decoded ``upload-timings.json`` record.
        """
        path = self.run.declared_root / "upload-timings.json"
        return json.loads(path.read_text(encoding="utf-8"))


def _refused_run(tmp_path: Path) -> RefusedRun:
    """Run the action against a release holding one asset with wrong bytes."""
    facts = release_facts()
    workspace = stage_workspace(tmp_path, facts)
    offender = min(recorded_digests(workspace))
    attached = [{"name": offender, "digest": f"sha256:{'0' * 64}"}]
    run = execute(tmp_path, workspace, facts, release=Release(attached=tuple(attached)))
    return RefusedRun(run=run, offender=offender)


def test_the_upload_step_logs_its_own_diagnostic(tmp_path: Path) -> None:
    """Print the refusal where the upload step runs, not only in its output."""
    refused = _refused_run(tmp_path)
    step = refused.run.completed[UPLOAD_STEP_INDEX]
    assert REFUSAL_PREFIX in step.stderr, step.stderr
    assert refused.offender in step.stderr, (
        "the logged diagnostic must name the asset that blocked the run"
    )


def test_the_upload_step_records_each_boundary_it_crossed(tmp_path: Path) -> None:
    """Retain a timings record beside the plan, with only crossed boundaries set."""
    facts = release_facts()
    workspace = stage_workspace(tmp_path, facts)
    run = execute(tmp_path, workspace, facts)
    assert run.outputs.get("upload-error") == "false", run.outputs.get("error-message")
    timings = json.loads(
        (run.declared_root / "upload-timings.json").read_text(encoding="utf-8")
    )
    # This release attached nothing, so there was nothing to download: the
    # boundary is `None`, not `0`, or a run that never downloaded would be
    # indistinguishable from an instantaneous one.
    assert timings["download_ms"] is None, timings
    for crossed in ("listing_ms", "upload_ms", "verification_ms"):
        assert isinstance(timings[crossed], int), timings


def test_a_refused_run_still_records_the_boundaries_it_crossed(tmp_path: Path) -> None:
    """Keep the timings of a run that refused: that is the run they matter for."""
    timings = _refused_run(tmp_path).timings()
    assert isinstance(timings["listing_ms"], int), timings
    # A refused run sends no transfer, so the boundary is absent rather than
    # recorded as a zero-duration upload that never happened.
    assert timings["upload_ms"] is None, timings


def test_the_upload_step_traces_each_call_it_made(tmp_path: Path) -> None:
    """Retain a per-call trace beside the plan, one record per ``gh`` call.

    The aggregate timings sum a boundary, so they cannot say which of a run's
    calls failed. The trace names each one, and the two files must describe the
    same run: a trace line with no timings beside it would be a different
    upload step's record.
    """
    facts = release_facts()
    workspace = stage_workspace(tmp_path, facts)
    run = execute(tmp_path, workspace, facts)
    lines = (run.declared_root / "upload-trace.jsonl").read_text(encoding="utf-8")
    records = [json.loads(line) for line in lines.splitlines()]
    assert [record["operation"] for record in records] == [
        "release_listing",
        "asset_upload",
        "release_verification",
    ], records
    for record in records:
        assert record["event"] == "release_upload_call", record
        assert record["outcome"] == "completed", record
        # The run identity links a line to the run that wrote it, and the
        # harness supplies none, so the bounded absence is what a local run
        # records rather than an omitted field.
        assert set(record) == TRACE_FIELDS, record
        assert isinstance(record["duration_seconds"], float), record
