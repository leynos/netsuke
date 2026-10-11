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
