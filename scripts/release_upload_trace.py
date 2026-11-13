"""Record one bounded trace event for every ``gh`` call the uploader makes.

The aggregate timings file answers "how long did the listing take". It cannot
answer "which of the eleven calls failed", because it sums each boundary into a
single number. An upload run crosses a process and network boundary every time
it lists the release, downloads an attached asset, or transfers one, and the
custom check requires tracing wherever a change crosses such a boundary. This
module is that trace.

Each record follows the repository's existing JSONL style: the ``record_type``
of ``trace_event``, a stable ``event`` name, the operation, an outcome, a
bounded error category, and a duration. Nothing here carries a URL, a payload
path, an asset name, or a command's output. A trace file outlives the run that
wrote it, and an asset name is both unbounded and attacker-visible in a way a
duration is not.

The runner seam is :class:`release_upload_timing.TimedRunner`, which already
wraps every ``gh`` invocation. Tracing there rather than at the call sites
means a boundary added later is traced without its author knowing this module
exists, and the two records cannot disagree about which calls happened.

Run via ``.github/actions/upload-declared-assets``; behavioural coverage lives
in ``scripts/tests/test_release_upload_trace.py``.
"""

import dataclasses
import json
import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import pathlib

__all__ = [
    "ERROR_CATEGORY_FAILED",
    "OUTCOME_COMPLETED",
    "OUTCOME_FAILED",
    "TRACE_NAME",
    "UNKNOWN",
    "CallReadings",
    "CallTrace",
    "TraceRecord",
    "operation_for_boundary",
    "record_for_call",
    "record_trace",
    "trace_context",
    "trace_path",
]

#: Name of the file the uploader writes beside its plan and timings.
TRACE_NAME = "upload-trace.jsonl"

#: The ``event`` name every record carries. Fixed because a consumer filters on
#: it, and a free-form value would make that filter useless.
CALL_EVENT = "release_upload_call"

#: The outcome of a call that returned to its caller.
OUTCOME_COMPLETED = "completed"

#: The outcome of a call that raised before returning.
OUTCOME_FAILED = "failed"

#: The two outcomes a record may carry, as the set the check below consults.
_OUTCOMES = frozenset({OUTCOME_COMPLETED, OUTCOME_FAILED})

#: The four boundaries a run can cross, each named for what it does to the
#: release rather than for the command that performs it. The uploader reaches
#: GitHub exactly these four ways, and a record naming anything else would not
#: describe a call this run can make.
OPERATIONS = frozenset({
    "release_listing",
    "asset_download",
    "asset_upload",
    "release_verification",
})

#: What a trace record carries in place of an identity the environment did not
#: supply. A local invocation has no workflow run, and recording the record with
#: an explicit absence is more useful than omitting it.
UNKNOWN = "unknown"

#: The fixed keys :func:`trace_context` fills. A consumer queries these by name,
#: so a key added here is a vocabulary change rather than an extra field.
_CONTEXT_KEYS = {
    "run_id": "GITHUB_RUN_ID",
    "job": "GITHUB_JOB",
    "run_attempt": "GITHUB_RUN_ATTEMPT",
}

#: What a failed call is attributed to. Deliberately one value: the runner sees
#: an exception, not its meaning, and inventing a finer category here would
#: report the interface's guess rather than the command's behaviour. The
#: uploader's own refusal table is what names a cause.
ERROR_CATEGORY_FAILED = "call_failed"


@dataclasses.dataclass(frozen=True, slots=True)
class TraceRecord:
    """Hold one bounded record of a single ``gh`` call.

    Attributes
    ----------
    operation
        Which upload boundary the call crossed: ``release_listing``,
        ``asset_download``, ``asset_upload`` or ``release_verification``. A
        bounded vocabulary, checked by :meth:`__post_init__`.
    outcome
        ``completed`` or ``failed``.
    error_category
        Why the call failed, or ``None`` when it did not.
    duration_seconds
        The measured duration, rounded to milliseconds of a second.
    timestamp_ns
        When the call finished, in nanoseconds since the epoch.
    context
        The run identity a record is linked through -- the workflow run id,
        the job, and the run attempt. Fixed keys, set by the caller.
    """

    operation: str
    outcome: str
    error_category: str | None
    duration_seconds: float
    timestamp_ns: int
    context: dict[str, str]

    def __post_init__(self) -> None:
        """Refuse an operation or outcome outside the bounded vocabulary."""
        if self.operation not in OPERATIONS:
            msg = f"unknown upload trace operation: {self.operation!r}"
            raise ValueError(msg)
        if self.outcome not in _OUTCOMES:
            msg = f"unknown upload trace outcome: {self.outcome!r}"
            raise ValueError(msg)

    def as_record(self) -> dict[str, object]:
        """Return the JSON object this record is written as.

        Returns
        -------
        dict[str, object]
            The bounded record, in the repository's existing trace style.
        """
        record: dict[str, object] = {
            "record_type": "trace_event",
            "event": CALL_EVENT,
            "operation": self.operation,
            "timestamp_ns": self.timestamp_ns,
            "outcome": self.outcome,
            "duration_seconds": self.duration_seconds,
            **self.context,
        }
        if self.error_category is not None:
            record["error_category"] = self.error_category
        return record


#: The timing boundary :class:`release_upload_timing.TimedRunner` reports to
#: the operation this trace names for it. The two vocabularies are separate on
#: purpose: one names a duration to sum, the other a call to trace.
_OPERATION_FOR_BOUNDARY = {
    "listing": "release_listing",
    "download": "asset_download",
    "upload": "asset_upload",
    "verification": "release_verification",
}


@dataclasses.dataclass(frozen=True, slots=True)
class CallTrace:
    """Hold the per-call records one run produced, in call order.

    Attributes
    ----------
    records
        One :class:`TraceRecord` per traced call.
    """

    records: tuple[TraceRecord, ...] = ()

    def appended(self, record: TraceRecord) -> CallTrace:
        """Return this trace with ``record`` added after the calls already held.

        Returns
        -------
        CallTrace
            The new trace. The receiver is unchanged, so a caller holding an
            earlier trace keeps the records it had.
        """
        return dataclasses.replace(self, records=(*self.records, record))


def trace_path(plan_path: pathlib.Path) -> pathlib.Path:
    """Return the trace destination beside a run's plan.

    Parameters
    ----------
    plan_path
        The plan destination the caller asked for.

    Returns
    -------
    pathlib.Path
        The trace file, in the plan's directory, so a job that retains one
        retains both.

    Examples
    --------
    >>> import pathlib
    >>> trace_path(pathlib.Path("artifacts/upload-plan.json")).name
    'upload-trace.jsonl'
    """
    return plan_path.parent / TRACE_NAME


def record_trace(plan_path: pathlib.Path, trace: CallTrace) -> None:
    """Write ``trace`` beside ``plan_path``.

    Parameters
    ----------
    plan_path
        The plan destination the caller asked for.
    trace
        The records the run produced.

    Notes
    -----
    An ``OSError`` propagates rather than being swallowed, for the same reason
    :func:`release_upload_timing_files.record_timings` propagates one: whether
    a run survives a missing diagnostic is the caller's decision, and this
    module cannot print a diagnostic about a diagnostic without deciding it
    here. It reaches the caller from the write below rather than being raised
    by name in a ``Raises`` section, which Ruff's DOC501 reserves for
    exceptions a function raises itself.

    Examples
    --------
    >>> import pathlib, tempfile
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     plan = pathlib.Path(tmp) / "upload-plan.json"
    ...     record_trace(plan, CallTrace())
    ...     trace_path(plan).read_text(encoding="utf-8")
    ''
    """
    destination = trace_path(plan_path)
    destination.write_text(
        "".join(json.dumps(record.as_record()) + "\n" for record in trace.records),
        encoding="utf-8",
    )


@dataclasses.dataclass(frozen=True, slots=True)
class CallReadings:
    """Hold the three clock readings one traced call is measured between.

    Attributes
    ----------
    started_ns
        The monotonic reading taken before the call.
    ended_ns
        The monotonic reading taken after it.
    timestamp_ns
        The wall-clock reading the record is stamped with.

    Notes
    -----
    They travel together because they are one measurement: a caller that
    paired the wrong start with an end would publish a plausible duration for
    a call that never took that long, and three positional integers invite
    exactly that.
    """

    started_ns: int
    ended_ns: int
    timestamp_ns: int


def record_for_call(
    *,
    boundary: str,
    outcome: str,
    readings: CallReadings,
    context: dict[str, str],
) -> TraceRecord:
    """Build the trace record one finished call is published as.

    Parameters
    ----------
    boundary
        The timing boundary the call crossed, which names the operation.
    outcome
        ``completed`` or ``failed``.
    readings
        The clock readings the call was measured between.
    context
        The run identity to link the record through.

    Returns
    -------
    TraceRecord
        The bounded record for that one call.

    Notes
    -----
    The duration is read from the raw nanoseconds rather than from a
    millisecond total: a call faster than a millisecond rounds to zero there,
    and a trace that reported every quick call as taking no time would hide
    exactly the calls a test watches for.

    Building the record here rather than inside the runner keeps the record
    vocabulary in the one module that defines it, so a field added to a trace
    record does not have to be threaded through a second file.

    Examples
    --------
    >>> record = record_for_call(
    ...     boundary="upload", outcome=OUTCOME_COMPLETED,
    ...     readings=CallReadings(
    ...         started_ns=0, ended_ns=2_500_000, timestamp_ns=1
    ...     ),
    ...     context={},
    ... )
    >>> record.operation, record.duration_seconds
    ('asset_upload', 0.0025)
    >>> record.error_category is None
    True
    """
    # Read in seconds from the raw nanoseconds rather than from a millisecond
    # aggregate: see the note above.
    elapsed_ns = readings.ended_ns - readings.started_ns
    return TraceRecord(
        operation=operation_for_boundary(boundary),
        outcome=outcome,
        error_category=(
            None if outcome == OUTCOME_COMPLETED else ERROR_CATEGORY_FAILED
        ),
        duration_seconds=round(max(0, elapsed_ns) / 1_000_000_000, 9),
        timestamp_ns=readings.timestamp_ns,
        context=context,
    )


def operation_for_boundary(boundary: str) -> str:
    """Return the trace operation a timing boundary is recorded as.

    Parameters
    ----------
    boundary
        A boundary name from :class:`release_upload_timing.TimedRunner`.

    Returns
    -------
    str
        The operation name this trace publishes for that boundary.

    Examples
    --------
    >>> operation_for_boundary("upload")
    'asset_upload'
    """
    return _OPERATION_FOR_BOUNDARY[boundary]


def trace_context(environment: cabc.Mapping[str, str]) -> dict[str, str]:
    """Return the run identity every record is linked through.

    Parameters
    ----------
    environment
        The process environment, supplied by the adapter that owns it. Nothing
        here reads ``os.environ``: the command line is the one place that
        legitimately owns process state, and a test supplies its own mapping.

    Returns
    -------
    dict[str, str]
        The run id, job name, and attempt, with ``unknown`` standing in for a
        value the environment does not carry.

    Examples
    --------
    >>> trace_context({"GITHUB_RUN_ID": "42", "GITHUB_JOB": "publish"})
    {'run_id': '42', 'job': 'publish', 'run_attempt': 'unknown'}
    """
    return {
        name: environment.get(variable) or UNKNOWN
        for name, variable in _CONTEXT_KEYS.items()
    }
