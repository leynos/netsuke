r"""Exercise the release-admission gate against degraded telemetry boundaries.

The three artefact channels of the gate do not fail alike, and the difference is
load-bearing rather than incidental. A metric write is fail-closed: an
unrecorded admission is not an admission, so a refused write replaces the
gate's own status. A trace write is fail-open: the observability channel must
not decide the admission, so a refused write is reported once and otherwise
ignored. Workflow outputs are fail-closed for the same reason metrics are.

This module pins the two asymmetries a *subsequent* observer can see -- a trace
sink that refused exactly one write, and a clock that refused exactly one read
-- because a boundary that fails for every call stops the chain before the
recovery the recorded flag is about. Those become state, not just a status.

Example (run from the repository root)::

    PYTHONPATH=scripts uv run --no-project --python 3.14 \
        --with pytest==9.0.2 --with hypothesis==6.151.9 \
        --with 'cmd-mox==0.2.0' --with 'cuprum==0.1.0' --with 'cyclopts==4.25.3' \
        python -m pytest scripts/tests/test_release_admission_metric_degradation.py \
        -c /dev/null --rootdir=. -p no:cacheprovider
"""

import typing as typ

import pytest
from release_admission_test_scenarios import fail_nth_call
from release_admission_test_support import (
    CANARY_BY_OPERATION,
    METRICS_VALIDATOR,
    expected_operation_labels,
    install_adapted,
    operation_duration,
    operation_records,
    run_gate,
)

if typ.TYPE_CHECKING:
    import pathlib

    from cmd_mox import CmdMox

pytest_plugins = ("cmd_mox.pytest_plugin",)

#: The diagnostic a refused trace write prints, exactly as the shell worded it.
TRACE_SINK_DIAGNOSTIC = "release-admission trace sink failed"
#: The delivery trace a recovered sink leaves as the last record.
TRACE_DELIVERY_FAILURE = {
    "event": "trace_delivery",
    "operation": "verify_evidence",
    "outcome": "failure",
    "error_category": "unknown",
    "duration_seconds": 0,
}
#: The operation both clock-read cases fail, and its labels under that failure.
CLOCK_OPERATION = "resolve_tag_commit"


def test_trace_sink_failure_preserves_gate_and_reports_recovery(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
) -> None:
    """Verify the trace sink is fail-open and reports bounded delivery failure.

    Notes
    -----
    Contract invariants: trace-sink failure is fail-open for the gate, and a
    bounded delivery-failure record is emitted once the sink recovers. A single
    refused write is enough to set the flag the final record reports, and every
    later write must still be attempted -- so the artefact keeps the records the
    sink refused, because the gate writes them in process as well.
    """
    adapted = install_adapted(cmd_mox)
    adapted.trace.wraps(fail_nth_call(1, "the trace sink refused\n"))
    run = run_gate(
        cmd_mox,
        tmp_path,
        evidence_state="fresh",
        extra_environment=adapted.variables(cmd_mox),
    )

    assert run.result.returncode == 0, run.result.stderr
    METRICS_VALIDATOR.validate_metrics(run.metrics)
    METRICS_VALIDATOR.validate_traces(run.traces)
    assert f"{TRACE_SINK_DIAGNOSTIC}\n" in run.result.stderr, (
        "a refused trace write must report its bounded diagnostic"
    )
    assert run.outputs["gate-outcome"] == "failure", (
        "a trace failure must not replace the observed admission result"
    )
    assert run.traces[-1] == TRACE_DELIVERY_FAILURE, (
        "a recovered trace sink must record its fixed delivery failure"
    )


@pytest.mark.parametrize("failure_read", [1, 2], ids=["start", "finish"])
def test_clock_failure_retains_bounded_operation_result(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
    failure_read: int,
) -> None:
    """Verify either clock read emits a bounded failed operation result.

    Parameters
    ----------
    cmd_mox
        Active controller whose shims stand in for every adapter.
    tmp_path
        Isolated output directory for the gate.
    failure_read
        The clock invocation, counting from one, that the double refuses.

    Notes
    -----
    Contract invariants: either clock failure produces an unknown operation
    result, a zero duration fallback, and a failed gate output. Both reads
    belong to the first operation, so the chain stops there either way, which is
    the shell's own behaviour: a failure at the start and a failure at the
    finish are indistinguishable in the records.
    """
    adapted = install_adapted(cmd_mox)
    adapted.clock.wraps(fail_nth_call(failure_read, "the clock failed\n"))
    run = run_gate(
        cmd_mox,
        tmp_path,
        evidence_state="fresh",
        extra_environment=adapted.variables(cmd_mox),
    )

    assert run.result.returncode == 0, run.result.stderr
    METRICS_VALIDATOR.validate_metrics(run.metrics)
    METRICS_VALIDATOR.validate_traces(run.traces)
    assert operation_records(run.metrics, CLOCK_OPERATION)[-1]["labels"] == (
        expected_operation_labels(
            CANARY_BY_OPERATION[CLOCK_OPERATION],
            CLOCK_OPERATION,
            "failure",
            "unknown",
        )
    ), "clock failure must retain a bounded operation failure"
    assert operation_duration(run.metrics, CLOCK_OPERATION) == 0, (
        "clock failure must retain the defined zero-duration fallback"
    )
    assert run.outputs["gate-outcome"] == "failure", (
        "clock failure must reach the workflow output boundary"
    )
    assert run.outputs["gate-error-category"] == "unknown", (
        "clock failure must retain the bounded unknown category"
    )
