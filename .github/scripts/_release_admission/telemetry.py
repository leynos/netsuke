"""Write the gate's bounded metric, trace, and workflow-output records.

Three artefacts leave this module, and each has a different failure contract:

- Metrics are fail-closed. A metric record that cannot be written aborts the
  gate result, because an unrecorded admission is not an admission.
- Traces are fail-open. A trace record that cannot be written is reported once
  per attempt and otherwise ignored, because the gate's own outcome must not
  depend on the observability channel.
- Workflow outputs are fail-closed, for the same reason as metrics: the job
  summary and the upload steps read them.

The distinction is not stylistic. It reproduces the shell's ``set -e``
behaviour exactly, including the asymmetry between its two write helpers: a
guarded trace write never changed the exit status, while an unguarded metric
or output write replaced the status with the write's own failure and abandoned
the rest of the gate record.

The record format itself lives in :mod:`records`; this module only decides
*whether* and *where* one is written, and what a failure costs.
"""

import dataclasses
import enum
import sys
import typing as typ

from . import delivery, policy
from .records import (
    WORKFLOW_OUTPUT_TRACE,
    GateRecord,
    MetricFields,
    OperationRecord,
    TraceFields,
    delivery_trace_fields,
    render_metric,
    render_trace,
)

if typ.TYPE_CHECKING:
    from pathlib import Path

#: The diagnostic printed when a label or value is outside its vocabulary.
VOCABULARY_DIAGNOSTIC = (
    "release-admission metric labels are outside the fixed vocabulary"
)

#: The diagnostic printed when a metric name is not one of the three.
NAME_DIAGNOSTIC = "release-admission metric name is outside the fixed vocabulary"

#: The diagnostic printed for a trace whose values are outside the vocabulary.
TRACE_DIAGNOSTIC = "release-admission trace values are outside the fixed vocabulary"

#: The diagnostic printed when a trace record could not be delivered.
TRACE_SINK_DIAGNOSTIC = "release-admission trace sink failed"

#: The four output keys, in the order the workflow reads them.
OUTPUT_KEYS: typ.Final[tuple[str, ...]] = (
    "gate-outcome",
    "gate-error-category",
    "metrics-file",
    "trace-file",
)


def diagnostic(message: str) -> None:
    """Write one bounded diagnostic to standard error.

    Every message this module prints is a fixed sentence from the list above.
    Nothing derived from the environment, a revision, a path, or an error body
    reaches a diagnostic, so a failure cannot leak an identifier into a runner
    log that is retained with the release.
    """
    sys.stderr.write(f"{message}\n")


class Refusal(enum.IntEnum):
    """Carry the status of a refused metric write to the gate's exit.

    Every non-zero value is a status ``set -e`` promoted to the gate's own: the
    sink's, when the adapter failed, or ``VOCABULARY`` when this module refused
    a record whose labels, name, or value fell outside the fixed vocabulary.
    The shell returned ``1`` from both of its refusal paths, so a refusal is
    indistinguishable from a plain command failure at the gate's exit.
    """

    WRITTEN = 0
    VOCABULARY = 1


@dataclasses.dataclass(frozen=True, slots=True)
class Sinks:
    """Hold where each artefact goes and how it is written.

    A sink adapter is the empty string when the gate should append in process.
    ``metrics_file`` and ``trace_file`` are absent only before the gate has
    created them, which is the state a missing required variable leaves; no
    record is written in that state.
    """

    metrics_file: Path | None
    trace_file: Path | None
    metrics_sink: str
    output_sink: str
    trace_sink: str


def emit_metric(sinks: Sinks, name: str, fields: MetricFields) -> Refusal:
    """Write one metric record, returning the status on which the gate stops.

    Every refusal is ``Refusal.VOCABULARY``, the ``1`` the shell returned from
    each of its own refusal arms, and the caller stops the gate on any non-zero
    value. That is deliberate: the shell's ``record_operation`` suspended
    abort-on-failure around its own calls with ``|| :``, so an operation's
    records could all be lost without changing its result, while
    ``record_gate_result`` let a refusal abort the gate record outright.

    Returns
    -------
    Refusal
        ``0`` when the record was written, otherwise the status the gate must
        stop with.
    """
    if not fields.is_valid():
        diagnostic(VOCABULARY_DIAGNOSTIC)
        return Refusal.VOCABULARY
    labels = _metric_labels(name, fields)
    if labels is None:
        diagnostic(NAME_DIAGNOSTIC)
        return Refusal.VOCABULARY
    record = render_metric(name, labels, fields.value)
    if record is None:
        diagnostic(NAME_DIAGNOSTIC)
        return Refusal.VOCABULARY
    status = delivery.append_record(sinks.metrics_sink, sinks.metrics_file, record)
    return Refusal(status) if status else Refusal.WRITTEN


def _metric_labels(name: str, fields: MetricFields) -> dict[str, str] | None:
    """Return the labels a metric name requires, or ``None`` for another name."""
    match name:
        case policy.GATE_METRIC:
            return {
                "outcome": fields.outcome,
                "error_category": fields.error_category,
            }
        case policy.OPERATION_METRIC:
            return {
                "canary": fields.canary,
                "operation": fields.operation,
                "outcome": fields.outcome,
                "error_category": fields.error_category,
            }
        case policy.DURATION_METRIC:
            return {"operation": fields.operation}
        case _:
            return None


def emit_trace(sinks: Sinks, fields: TraceFields) -> bool:
    """Write one trace record, fail-open, returning the trace-sink state.

    A non-zero status from :func:`delivery.append_record` is the only way this
    reports a failed delivery. Reading it as a boolean would invert the check,
    because the success status is ``0``.

    Returns
    -------
    bool
        Whether trace delivery has failed so far. A record outside the
        vocabulary and an unwritable record both set it; neither stops the
        gate.
    """
    record = render_trace(fields)
    if record is None:
        diagnostic(TRACE_DIAGNOSTIC)
        return True
    if delivery.append_record(sinks.trace_sink, sinks.trace_file, record) != 0:
        diagnostic(TRACE_SINK_DIAGNOSTIC)
        return True
    return False


def record_operation(sinks: Sinks, record: OperationRecord) -> bool:
    """Write one operation's counter, duration, and completion trace.

    The shell ran these three writes inside a call it negated with ``!``,
    which suspended its abort-on-failure rule for the whole operation. A
    metric sink that refuses every record therefore loses the records and
    nothing else: the operation's own result, and the gate's, are unaffected.

    Returns
    -------
    bool
        Whether the operation's completion trace failed to deliver. The caller
        accumulates it, because a trace write is fail-open and must not decide
        the operation's result.
    """
    emit_metric(sinks, policy.OPERATION_METRIC, record.counter_fields())
    emit_metric(sinks, policy.DURATION_METRIC, record.duration_fields())
    return emit_trace(sinks, record.trace_fields())


def record_gate_result(
    sinks: Sinks,
    *,
    output: Path | None,
    record: GateRecord,
    trace_sink_failed: bool,
) -> int:
    """Write the final gate record: counter, trace, outputs, and delivery traces.

    Returns
    -------
    int
        ``0`` when the gate record was written, otherwise the status on which
        the gate must stop. The shell's exit trap aborted at its first failing
        unguarded write, and ``set -e`` then replaced the exit status with that
        write's own status, so the caller treats a non-zero value as a failed
        gate rather than as a lost record.
    """
    status = emit_metric(sinks, policy.GATE_METRIC, record.metric_fields())
    if status:
        return status
    failed = emit_trace(sinks, record.trace_fields())
    trace_sink_failed = trace_sink_failed or failed
    for key, value in (
        ("gate-outcome", record.outcome),
        ("gate-error-category", record.error_category),
        ("metrics-file", _rendered_path(sinks.metrics_file)),
        ("trace-file", _rendered_path(sinks.trace_file)),
    ):
        status = write_output(sinks, output, f"{key}={value}")
        if status:
            return status
    failed = emit_trace(sinks, WORKFLOW_OUTPUT_TRACE)
    trace_sink_failed = trace_sink_failed or failed
    emit_trace(sinks, delivery_trace_fields(trace_sink_failed=trace_sink_failed))
    return 0


def _rendered_path(path: Path | None) -> str:
    """Render an artefact path for a workflow output, empty when unset."""
    return "" if path is None else str(path)


def write_output(sinks: Sinks, output: Path | None, record: str) -> int:
    """Write one workflow-output line through the output sink.

    The output sink is its own adapter variable and its target is the
    ``GITHUB_OUTPUT`` file. The shell's ``write_workflow_output`` helper was a
    copy of the metric helper, but it was called with the output adapter, not
    the metric one; a run that configured only a metric sink therefore appended
    its outputs in process, which is the behaviour reproduced here.

    Returns
    -------
    int
        ``0`` for a write that succeeded, otherwise the status on which the
        gate must stop. A run with no output file is refused outright, which is
        the state a required variable left behind before the artefact paths
        existed.
    """
    if output is None:
        return int(Refusal.VOCABULARY)
    status = delivery.append_record(sinks.output_sink, output, record)
    return status or 0
