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
from release_admission_test_scenarios import fail_first_calls, fail_nth_call
from release_admission_test_support import (
    CANARY_BY_OPERATION,
    METRICS_VALIDATOR,
    assert_failure_trace_sequence,
    expected_gate_labels,
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
#: The status a sink may choose, which is neither success nor this module's own
#: refusal. It exists to be adopted verbatim, so it must not be ``0`` or ``1``.
SINK_STATUS = 2
#: A missing-evidence run stops at ``check_scan_freshness``, so four operations
#: run and each makes two metric writes -- a counter and a duration. Those eight
#: writes are the guarded ones; the gate's own counter is the ninth and is not.
GUARDED_METRIC_WRITES = 8


def test_operation_metric_writes_lose_only_their_records(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
) -> None:
    """Verify a sink status the gate does not own is adopted, not raised.

    Notes
    -----
    Contract invariants: a sink may exit with any status, and the gate adopts
    what it chose rather than narrowing it to the refusal vocabulary. Refusing
    exactly the guarded operation writes and letting the gate's own write
    through is what separates the two regimes: the records are lost, the
    admission result and its exit status are not, and the gate record is still
    written.

    The status is ``2`` rather than ``1`` on purpose. Narrowing a sink's status
    into the refusal enum raises ``ValueError`` for anything but ``0`` and ``1``,
    so a sink that exits ``2`` is the one case where the narrowing is visible as
    an uncaught traceback instead of a lost record.
    """
    adapted = install_adapted(cmd_mox)
    adapted.metrics.wraps(fail_first_calls(GUARDED_METRIC_WRITES, SINK_STATUS))
    run = run_gate(
        cmd_mox,
        tmp_path,
        evidence_state="missing",
        extra_environment=adapted.variables(cmd_mox),
    )

    assert "Traceback" not in run.result.stderr, (
        "a sink's own exit status must be adopted, never narrowed into a raise"
    )
    assert run.result.returncode == 0, run.result.stderr
    assert len(adapted.metrics.calls) == GUARDED_METRIC_WRITES + 1, (
        "the case must refuse exactly the guarded operation writes"
    )
    assert len(run.metrics) == 1, (
        "the guarded operation records are lost; only the gate's own survives"
    )
    assert run.metrics[0]["name"] == "netsuke_release_admission_gate_total", (
        "the surviving record must be the gate's own counter, not an operation's"
    )
    assert run.metrics[0]["labels"] == expected_gate_labels(
        "failure", "missing_evidence"
    ), "the gate record must still report the observed admission"
    assert_failure_trace_sequence(
        run.traces, "check_scan_freshness", "missing_evidence"
    )
    assert tuple(run.outputs) == (
        "gate-outcome",
        "gate-error-category",
        "metrics-file",
        "trace-file",
    ), "a refused operation write must not cost any workflow output"
    assert run.outputs["gate-outcome"] == "failure", (
        "the refused operation writes must not change the observed admission"
    )
    assert run.outputs["gate-error-category"] == "missing_evidence", (
        "the refused operation writes must not change the reported category"
    )
    assert run.outputs["metrics-file"] == str(run.paths["metrics"]), (
        "the workflow must still name the metric artefact"
    )
    assert run.outputs["trace-file"] == str(run.paths["trace"]), (
        "the workflow must still name the trace artefact"
    )
    METRICS_VALIDATOR.validate_metrics(run.metrics)
    METRICS_VALIDATOR.validate_traces(run.traces)


def test_gate_exits_with_the_status_its_own_metric_write_reported(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
) -> None:
    """Verify the gate adopts an unguarded sink status as its own exit status.

    Notes
    -----
    Contract invariants: the shell's ``set -e`` replaced the gate's exit status
    with the failing write's own status, so a sink that exits ``2`` leaves the
    step exiting ``2``. The gate's counter is the first unguarded metric write,
    which is why refusing it is the observation that pins the behaviour.

    Wording and classification are untouched here: the sink prints nothing in
    this suite, so the gate's own stderr is the same silence the shell produced.
    """
    adapted = install_adapted(cmd_mox)
    adapted.metrics.wraps(fail_first_calls(GUARDED_METRIC_WRITES + 1, SINK_STATUS))
    run = run_gate(
        cmd_mox,
        tmp_path,
        evidence_state="missing",
        extra_environment=adapted.variables(cmd_mox),
    )

    assert "Traceback" not in run.result.stderr, (
        "a sink's own exit status must be adopted, never narrowed into a raise"
    )
    assert run.result.returncode == SINK_STATUS, run.result.stderr
    assert run.metrics == [], "the refused gate record must not appear in the artefact"
    assert run.outputs == {}, (
        "a refused gate record must abandon the workflow outputs behind it"
    )
    assert [trace["event"] for trace in run.traces][-1] == "operation_complete", (
        "the gate-completion traces must not outrun the refused gate record"
    )


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
    later write must still be attempted. The refused record itself is lost: a
    configured sink replaces the in-process append rather than adding to it, so
    the artefact holds the later records and the final delivery failure.
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
