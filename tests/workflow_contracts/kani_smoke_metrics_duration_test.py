"""Check proof-duration handling when Kani execution is skipped."""

import sys
import typing as typ

from workflow_loading import REPO_ROOT

if typ.TYPE_CHECKING:
    from pathlib import Path

    import pytest

sys.path.insert(0, str(REPO_ROOT / "scripts"))

import kani_smoke_metrics as metrics


def test_skipped_workflow_omits_duration_without_warning(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Omit timing quietly when the proof guard prevented proof execution."""
    state_directory = tmp_path / "state"
    state_directory.mkdir()
    records: list[dict[str, object]] = []
    inputs = metrics.WorkflowInputs(
        event_name="pull_request",
        job_status="success",
        scope_outcome="success",
        run_proofs="false",
        cache_outcome="skipped",
        cache_hit="",
        proofs_outcome="skipped",
        mutations_outcome="skipped",
    )
    metrics.finalize(
        inputs,
        metrics.FinalizerFiles(
            str(state_directory),
            str(tmp_path / "decision"),
            str(tmp_path / "out"),
        ),
        metrics.FinalizerDependencies(
            clock=lambda: 100, sink=lambda _path, values: records.extend(values)
        ),
    )
    assert len(records) == 2, "a guarded skip must emit counters without duration"
    assert not capsys.readouterr().err, (
        "an absent start mark is expected when proofs are skipped"
    )


def test_failed_end_mark_omits_duration_after_later_work(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Avoid including mutation and cache work when a mark write failed."""
    state_directory = tmp_path / "state"
    state_directory.mkdir()
    (state_directory / metrics.MARK_START_FILE).write_text("50\n", encoding="ascii")
    records: list[dict[str, object]] = []
    metrics.finalize(
        metrics.WorkflowInputs(
            event_name="pull_request",
            job_status="success",
            scope_outcome="success",
            run_proofs="true",
            cache_outcome="success",
            cache_hit="true",
            proofs_outcome="success",
            mutations_outcome="success",
            end_mark_recorded="false",
            end_mark_outcome="success",
        ),
        metrics.FinalizerFiles(
            str(state_directory),
            str(tmp_path / "decision"),
            str(tmp_path / "out"),
        ),
        metrics.FinalizerDependencies(
            clock=lambda: 9_000_000_050,
            sink=lambda _path, values: records.extend(values),
        ),
    )
    assert len(records) == 2, "a failed end mark must omit the untrustworthy duration"
    assert capsys.readouterr().err == metrics.WARNING_DURATION + "\n", (
        "a failed end mark must produce one fixed duration warning"
    )


def test_cancelled_end_mark_failure_does_not_use_fallback(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Avoid fallback when cancellation follows a recorded mark failure."""
    state_directory = tmp_path / "state"
    state_directory.mkdir()
    (state_directory / metrics.MARK_START_FILE).write_text("50\n", encoding="ascii")
    records: list[dict[str, object]] = []
    metrics.finalize(
        metrics.WorkflowInputs(
            event_name="pull_request",
            job_status="cancelled",
            scope_outcome="success",
            run_proofs="true",
            cache_outcome="cancelled",
            cache_hit="",
            proofs_outcome="cancelled",
            mutations_outcome="skipped",
            end_mark_recorded="false",
            end_mark_outcome="cancelled",
        ),
        metrics.FinalizerFiles(
            str(state_directory),
            str(tmp_path / "decision"),
            str(tmp_path / "out"),
        ),
        metrics.FinalizerDependencies(
            clock=lambda: 9_000_000_050,
            sink=lambda _path, values: records.extend(values),
        ),
    )
    assert len(records) == 2, "a failed end mark must suppress fallback duration"
    assert capsys.readouterr().err == metrics.WARNING_DURATION + "\n", (
        "a cancelled failed marker must warn without using finalizer time"
    )
