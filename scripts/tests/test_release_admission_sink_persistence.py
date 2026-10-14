r"""Pin the gate's behaviour when a record sink refuses *every* write.

:mod:`test_release_admission_metric_degradation` exercises the asymmetries a
sink that *recovers* exposes: a refused trace write is reported once and a
later record can still say delivery failed. Neither of the persistent cases is
reachable from those: a sink that never recovers stops the chain at the write
that owns it, so the behaviour is the boundary's own failure contract rather
than a recovery.

The two cases are deliberately asymmetric, and the asymmetry is the whole
point:

- The output sink is fail-closed. Its writes sit between ``gate_complete`` and
  the delivery traces, so a refused write abandons the records behind it and
  ``set -e`` replaces the gate's exit status with the sink's own.
- The trace sink is fail-open. Every attempted write is reported once and
  otherwise ignored, so the gate ends exactly as it would have without the
  sink, and the delivery traces are lost with the rest.

The measured counts come from :mod:`release_admission_test_support`, whose
frozen-behaviour section records both runs against the Bash gate; the status is
the one property this module states more strictly, because a sink exiting ``2``
is what makes "the gate adopted the sink's status" distinguishable from "the
gate reported its own failure".

Example (run from the repository root)::

    PYTHONPATH=scripts uv run --no-project --python 3.14 \
        --with pytest==9.0.2 --with hypothesis==6.151.9 \
        --with 'cmd-mox==0.2.0' --with 'cuprum==0.1.0' --with 'cyclopts==4.25.3' \
        python -m pytest scripts/tests/test_release_admission_sink_persistence.py \
        -c /dev/null --rootdir=. -p no:cacheprovider
"""

import typing as typ

from release_admission_test_support import (
    METRICS_VALIDATOR,
    Wrapper,
    install_adapted,
    run_gate,
)

if typ.TYPE_CHECKING:
    import pathlib

    from cmd_mox import CmdMox
    from cmd_mox.ipc import Invocation
    from release_admission_test_doubles import Handler

pytest_plugins = ("cmd_mox.pytest_plugin",)

#: The status a sink may choose, which is neither success nor this suite's own
#: refusal. It exists to be adopted verbatim, so it must not be ``0`` or ``1``.
SINK_STATUS = 2
#: The frozen measurements both persistent cases share: a fresh-evidence run
#: completes all five operations, so five counters and five durations precede
#: the gate's own counter. The five traces are the operations' completion
#: records, and ``gate_complete`` is the sixth.
COMPLETE_METRIC_RECORDS = 11
OPERATION_TRACES = 5
#: The traced record the gate writes with its final counter, which is where the
#: output sink's writes begin and the trace sink's delivery traces end.
GATE_TRACE_EVENT = "gate_complete"
#: The delivery traces the gate attempts after its workflow outputs: the
#: workflow-output record and the delivery verdict. A persistent trace sink
#: therefore reports one diagnostic per attempted write, which is the five
#: operation records, ``gate_complete``, and these two.
DELIVERY_TRACES = 2
#: The evidence state both cases run under. ``fresh`` is what lets all five
#: operations complete without the freshness refusal stopping the chain early.
FRESH_EVIDENCE = "fresh"


def refuse_every_call(message: str, status: int) -> Wrapper:
    """Return a wrapper that refuses every invocation of a boundary.

    A sink that fails for its whole lifetime is a different observation from
    one that refuses a bounded prefix: nothing recovers, so the gate's own
    failure contract for that boundary is what the run ends on rather than the
    recovery a later record would report. The wrappers in
    :mod:`release_admission_test_scenarios` are the recovering shape; this one
    is here because only the persistent cases below use it.

    Returns
    -------
    Wrapper
        A wrapper that answers every invocation with the refusal.
    """

    def wrapper(invocation: Invocation, handler: Handler) -> tuple[str, str, int]:
        """Refuse the invocation, ignoring what it asked for."""
        del invocation, handler
        return "", message, status

    return wrapper


def test_persistent_output_sink_failure_abandons_the_delivery_traces(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
) -> None:
    """Verify a refused workflow-output write stops the gate record mid-way.

    Notes
    -----
    Contract invariants: the gate's counter is written before the outputs, the
    four output writes precede the delivery traces, and ``set -e`` adopted the
    failing write's own status as the gate's. A sink that refuses every write
    therefore leaves a complete metric artefact, a trace artefact that stops at
    ``gate_complete``, and no workflow output at all -- the records behind the
    refusal are abandoned, not attempted.

    The sink's status is ``2`` so that adoption is visible: a gate that reported
    its own failure would exit ``1``, which is the value the frozen measurement
    recorded for a sink that chose ``1``.

    The sink's own diagnostic is asserted too. The shell ran the adapter as an
    ordinary command with both of its streams unredirected, so its message
    reached the runner log; a port that captured or discarded the adapter's
    standard error would satisfy every count above and still lose it.
    """
    adapted = install_adapted(cmd_mox)
    diagnostic = "the output sink refused every record"
    adapted.output.wraps(refuse_every_call(f"{diagnostic}\n", SINK_STATUS))
    run = run_gate(
        cmd_mox,
        tmp_path,
        evidence_state=FRESH_EVIDENCE,
        extra_environment=adapted.variables(cmd_mox),
    )

    assert "Traceback" not in run.result.stderr, (
        "a sink's own exit status must be adopted, never narrowed into a raise"
    )
    assert run.result.returncode == SINK_STATUS, run.result.stderr
    assert f"{diagnostic}\n" in run.result.stderr, (
        "an operator's adapter diagnostic must reach the runner log"
    )
    METRICS_VALIDATOR.validate_metrics(run.metrics)
    assert len(run.metrics) == COMPLETE_METRIC_RECORDS, (
        "the metric artefact must be complete through the gate's own counter"
    )
    assert run.metrics[-1]["name"] == "netsuke_release_admission_gate_total", (
        "the gate counter is written before the outputs, so it must survive"
    )
    assert len(run.traces) == OPERATION_TRACES + 1, (
        "the trace artefact must stop at gate_complete, before the output writes"
    )
    assert [trace["event"] for trace in run.traces][-1] == GATE_TRACE_EVENT, (
        "no record behind the refused output write may be attempted"
    )
    assert run.outputs == {}, (
        "a refused output write must leave the workflow output file unwritten"
    )


def test_persistent_trace_sink_failure_is_fail_open_for_the_gate(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
) -> None:
    """Verify a trace sink that never recovers costs only the trace artefact.

    Notes
    -----
    Contract invariants: trace delivery is fail-open, every attempted write is
    reported once, and the gate's own result is untouched. The gate's status is
    decided by the *admission*, so a refused trace write leaves the exit status
    and the four workflow outputs exactly as a healthy sink would, and the
    trace artefact empty because a configured sink replaces the in-process
    append rather than adding to it.

    The diagnostic count is the number of attempted trace writes -- the five
    operation records, ``gate_complete``, and the two delivery traces -- which
    is what distinguishes "reported once per attempt" from "reported once".
    """
    adapted = install_adapted(cmd_mox)
    diagnostic = "the trace sink refused every record"
    adapted.trace.wraps(refuse_every_call(f"{diagnostic}\n", SINK_STATUS))
    run = run_gate(
        cmd_mox,
        tmp_path,
        evidence_state=FRESH_EVIDENCE,
        extra_environment=adapted.variables(cmd_mox),
    )

    assert "Traceback" not in run.result.stderr, (
        "a refused trace write must be reported, never raised"
    )
    assert run.result.returncode == 0, run.result.stderr
    assert run.result.stderr.count(diagnostic) == (
        OPERATION_TRACES + DELIVERY_TRACES + 1
    ), "a trace write must report its refusal once per attempt, and none is skipped"
    METRICS_VALIDATOR.validate_metrics(run.metrics)
    assert len(run.metrics) == COMPLETE_METRIC_RECORDS, (
        "a refused trace write must not cost the metric artefact anything"
    )
    assert run.traces == [], (
        "the trace artefact must hold no record while its sink refuses them"
    )
    assert tuple(run.outputs) == (
        "gate-outcome",
        "gate-error-category",
        "metrics-file",
        "trace-file",
    ), "a refused trace write must not cost any workflow output"
    assert run.outputs["gate-outcome"] == "failure", (
        "the observed admission must be reported unchanged"
    )
    assert run.outputs["gate-error-category"] == "missing_evidence", (
        "the reported category must be the admission's own"
    )
    assert run.outputs["trace-file"] == str(run.paths["trace"]), (
        "the workflow must still name the trace artefact"
    )
