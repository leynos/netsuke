"""Render the gate's bounded metric and trace records as JSON Lines.

This module is pure: it validates values against the fixed vocabularies and
turns them into text, and it performs no I/O at all. Keeping it separate from
:mod:`telemetry` is what lets the record format be tested without a
filesystem, and it is the only place a record's bytes are decided.

Two value objects carry the shell's own argument vectors -- the five metric
positions and the five trace positions -- and two more compose them into the
exact record set one operation, or the gate itself, produces. Grouping them is
what keeps every entry point to four parameters or fewer without losing a
single position the shell validated.

Records are assembled by splicing the already-validated numeric text into a
:mod:`json` rendering of the remaining fields. That is not a shortcut around
the encoder: the shell interpolated the value unquoted, so every published
record carries a JSON *number*, and ``json.dumps`` would emit a string for the
text Python holds. The vocabulary check runs first and accepts only digits with
an optional fraction, so the splice cannot introduce syntax.
"""

import dataclasses
import json
import typing as typ

from . import policy


@dataclasses.dataclass(frozen=True, slots=True)
class MetricFields:
    """Hold the five metric positions the shell validated for every record.

    The names are the shell's own: ``canary`` and ``operation`` label the
    counter, ``outcome`` and ``error_category`` classify it, and ``value`` is
    the rendered number. The duration metric and the gate counter leave some of
    those positions unused, and the shell still validated all five for every
    name, so the group is deliberately uniform rather than per-metric.
    """

    canary: str
    operation: str
    outcome: str
    error_category: str
    value: str

    def is_valid(self) -> bool:
        """Return whether all five fields are inside the fixed vocabulary.

        A value is validated as *text*: a duration Python renders in exponent
        form fails, exactly as the shell's own pattern failed it.

        Returns
        -------
        bool
            Whether every field is inside its vocabulary.

        Examples
        --------
        >>> MetricFields("none", "verify_evidence", "success", "none", "1")
        ...     .is_valid()
        True
        >>> MetricFields("none", "verify_evidence", "success", "none", "1e-05")
        ...     .is_valid()
        False
        """
        return (
            policy.is_canary(self.canary)
            and policy.is_operation(self.operation)
            and policy.is_outcome(self.outcome)
            and policy.is_error_category(self.error_category)
            and policy.is_metric_value(self.value)
        )


@dataclasses.dataclass(frozen=True, slots=True)
class TraceFields:
    """Hold the five trace positions the shell validated before rendering.

    The first position is the trace *event* rather than a canary, and the last
    is the rendered duration rather than a metric value, because a trace names
    the boundary it observed instead of the counter it contributes to.
    """

    event: str
    operation: str
    outcome: str
    error_category: str
    duration: str

    def is_valid(self) -> bool:
        """Return whether all five fields are inside the fixed vocabulary.

        Returns
        -------
        bool
            Whether every field is inside its vocabulary.

        Examples
        --------
        >>> TraceFields(
        ...     "operation_complete", "verify_evidence", "success", "none", "1"
        ... ).is_valid()
        True
        """
        return (
            policy.is_trace_event(self.event)
            and policy.is_operation(self.operation)
            and policy.is_outcome(self.outcome)
            and policy.is_error_category(self.error_category)
            and policy.is_metric_value(self.duration)
        )


@dataclasses.dataclass(frozen=True, slots=True)
class OperationRecord:
    """Hold the record set one admission operation produces.

    The shell wrote three records per operation from one set of variables: the
    operation counter, the duration observation, and the completion trace. The
    duration metric carries no canary and no classification of its own, and the
    counter's value is always ``1``, so the three derivations live here rather
    than being restated at every call site.
    """

    canary: str
    operation: str
    outcome: str
    error_category: str
    duration: str

    def counter_fields(self) -> MetricFields:
        """Return the fields for the operation counter, whose value is ``1``."""
        return MetricFields(
            self.canary, self.operation, self.outcome, self.error_category, "1"
        )

    def duration_fields(self) -> MetricFields:
        """Return the fields for the duration metric, which carries no canary."""
        return MetricFields(
            policy.Canary.NONE,
            self.operation,
            policy.Outcome.UNKNOWN,
            policy.ErrorCategory.UNKNOWN,
            self.duration,
        )

    def trace_fields(self) -> TraceFields:
        """Return the fields for the operation's completion trace."""
        return TraceFields(
            policy.TRACE_OPERATION,
            self.operation,
            self.outcome,
            self.error_category,
            self.duration,
        )


@dataclasses.dataclass(frozen=True, slots=True)
class GateRecord:
    """Hold the record set the gate writes for its own final result.

    Every trace the gate emits names ``verify_evidence`` as its operation,
    because the gate's result is what that operation was deciding. The three
    traces differ only in their event and in what they report about delivery,
    so those derivations belong beside the value rather than in the writer.
    """

    outcome: str
    error_category: str

    def metric_fields(self) -> MetricFields:
        """Return the fields for the gate counter, whose value is ``1``."""
        return MetricFields(
            policy.Canary.NONE,
            policy.Operation.VERIFY_EVIDENCE,
            self.outcome,
            self.error_category,
            "1",
        )

    def trace_fields(self) -> TraceFields:
        """Return the fields for the gate-completion trace."""
        return TraceFields(
            policy.TRACE_GATE,
            policy.Operation.VERIFY_EVIDENCE,
            self.outcome,
            self.error_category,
            "0",
        )


#: The trace the shell wrote once its four workflow-output lines were written.
#: It reports delivery, not admission, so it is unconditional success and does
#: not belong to any gate result.
WORKFLOW_OUTPUT_TRACE: typ.Final[TraceFields] = TraceFields(
    policy.TRACE_WORKFLOW_OUTPUT,
    policy.Operation.VERIFY_EVIDENCE,
    policy.Outcome.SUCCESS,
    policy.ErrorCategory.NONE,
    "0",
)


def delivery_trace_fields(*, trace_sink_failed: bool) -> TraceFields:
    """Return the fields for the trace-delivery trace.

    The trace-delivery record is the one place the gate reports whether its own
    observability channel worked, so it carries the failure the fail-open trace
    writes have accumulated rather than the gate's own result. It derives
    nothing from the gate record, which is why it sits beside the value object
    rather than on it.

    Returns
    -------
    TraceFields
        Success when every trace was delivered, and the failure the shell
        reported when one was not.

    Examples
    --------
    >>> delivery_trace_fields(trace_sink_failed=True).outcome
    <Outcome.FAILURE: 'failure'>
    >>> delivery_trace_fields(trace_sink_failed=False).error_category
    <ErrorCategory.NONE: 'none'>
    """
    if trace_sink_failed:
        return TraceFields(
            policy.TRACE_DELIVERY,
            policy.Operation.VERIFY_EVIDENCE,
            policy.Outcome.FAILURE,
            policy.ErrorCategory.UNKNOWN,
            "0",
        )
    return TraceFields(
        policy.TRACE_DELIVERY,
        policy.Operation.VERIFY_EVIDENCE,
        policy.Outcome.SUCCESS,
        policy.ErrorCategory.NONE,
        "0",
    )


def render_metric(name: str, labels: dict[str, str], value: str) -> str | None:
    """Return one JSON Lines metric record, or ``None`` for an unknown name.

    Returns
    -------
    str | None
        The rendered record, or ``None`` when the name is not one of the three.
        A caller validates the vocabulary first; this function has no side
        effects, so the gate's decision stays separable from its output.

    Examples
    --------
    >>> render_metric(
    ...     policy.GATE_METRIC,
    ...     {"outcome": "failure", "error_category": "mismatch"},
    ...     "1",
    ... )
    '{"name":"netsuke_release_admission_gate_total","labels":{"outcome":"failure","error_category":"mismatch"},"value":1}'
    """
    if name not in METRIC_NAMES:
        return None
    head = json.dumps({"name": name, "labels": labels}, separators=(",", ":"))
    return f'{head[:-1]},"value":{value}}}'


#: The three metric names, as the shell's ``case`` accepted them.
METRIC_NAMES: typ.Final[frozenset[str]] = frozenset((
    policy.GATE_METRIC,
    policy.OPERATION_METRIC,
    policy.DURATION_METRIC,
))


def render_trace(fields: TraceFields) -> str | None:
    """Return one JSON Lines trace record, or ``None`` when it is invalid.

    As with :func:`render_metric`, the duration is spliced in as raw text so
    the trace carries a JSON number rather than a quoted string.

    Returns
    -------
    str | None
        The rendered record, or ``None`` when a field is outside its
        vocabulary.

    Examples
    --------
    >>> render_trace(
    ...     TraceFields("operation_complete", "verify_evidence", "success", "none", "1")
    ... )
    '{"event":"operation_complete","operation":"verify_evidence","outcome":"success","error_category":"none","duration_seconds":1}'
    """
    if not fields.is_valid():
        return None
    head = json.dumps(
        {
            "event": fields.event,
            "operation": fields.operation,
            "outcome": fields.outcome,
            "error_category": fields.error_category,
        },
        separators=(",", ":"),
    )
    return f'{head[:-1]},"duration_seconds":{fields.duration}}}'
