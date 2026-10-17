#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# dependencies = [
#   "cuprum==0.1.0",
#   "cyclopts==4.25.3",
# ]
# ///
"""Write bounded metrics for one `kani-smoke` job attempt.

The scope step writes its fixed decision to a separate record. Start and end
commands capture monotonic timestamps around `make kani-ir`; the finalizer
combines those inputs with step outcomes and atomically writes a JSONL artifact.
Uncertain outcomes remain observable as fixed `unknown` values.
"""

import contextlib
import dataclasses
import json
import math
import sys
import time
import typing as typ
from pathlib import Path

import cyclopts
from cyclopts import App, Parameter

app = App(help=__doc__, config=cyclopts.config.Env("INPUT_", command=False))

EVENTS = frozenset({"pull_request", "push", "schedule", "workflow_dispatch"})
DECISIONS = frozenset({
    "full_run",
    "in_scope",
    "out_of_scope",
    "unreadable_diff",
    "invalid_scope",
})
OUTCOMES = frozenset({
    "cancelled",
    "scope_failure",
    "skipped",
    "invalid_decision",
    "proof_failure",
    "proof_not_run",
    "mutation_failure",
    "job_failure",
    "success",
    "unknown",
})
CACHE_RESULTS = frozenset({"hit", "miss", "not_attempted", "unknown"})
MARK_START_FILE = "proof-start-ns"
MARK_END_FILE = "proof-end-ns"
METRICS_FILE = "metrics.jsonl"
WARNING_MARK_WRITE = "::warning::Kani proof timing mark could not be recorded."
WARNING_DURATION = "::warning::Kani proof duration could not be measured."
WARNING_END_FALLBACK = "::warning::Kani end mark missing; finalizer time used."
ERROR_METRICS_WRITE = "::error::Kani smoke metrics could not be written."


@Parameter(name="*")
@dataclasses.dataclass(frozen=True, slots=True)
class WorkflowInputs:
    """Carry the bounded workflow values used by the finalizer."""

    event_name: typ.Annotated[str, Parameter(env_var="INPUT_EVENT_NAME")]
    job_status: typ.Annotated[str, Parameter(env_var="INPUT_JOB_STATUS")]
    scope_outcome: typ.Annotated[str, Parameter(env_var="INPUT_SCOPE_OUTCOME")]
    run_proofs: typ.Annotated[str, Parameter(env_var="INPUT_RUN_PROOFS")]
    cache_outcome: typ.Annotated[str, Parameter(env_var="INPUT_CACHE_OUTCOME")]
    cache_hit: typ.Annotated[str, Parameter(env_var="INPUT_CACHE_HIT")]
    proofs_outcome: typ.Annotated[str, Parameter(env_var="INPUT_PROOFS_OUTCOME")]
    mutations_outcome: typ.Annotated[str, Parameter(env_var="INPUT_MUTATIONS_OUTCOME")]
    end_mark_recorded: typ.Annotated[
        str, Parameter(env_var="INPUT_END_MARK_RECORDED")
    ] = ""
    end_mark_outcome: typ.Annotated[
        str, Parameter(env_var="INPUT_END_MARK_OUTCOME")
    ] = ""


@Parameter(name="*")
@dataclasses.dataclass(frozen=True, slots=True)
class FinalizerFiles:
    """Name the state, decision and artifact files used by one finalizer."""

    state_directory: typ.Annotated[str, Parameter(env_var="INPUT_STATE_DIRECTORY")]
    decision_file: typ.Annotated[str, Parameter(env_var="INPUT_DECISION_FILE")]
    metrics_file: typ.Annotated[str, Parameter(env_var="INPUT_METRICS_FILE")]


class MonotonicClock(typ.Protocol):
    """Define the injected monotonic nanosecond clock used by the finalizer."""

    def __call__(self) -> int:
        """Return a monotonic timestamp in nanoseconds."""


class MetricsSink(typ.Protocol):
    """Define the atomic file sink used to retain metric records."""

    def __call__(self, path: Path, records: list[dict[str, object]], /) -> None:
        """Write complete metric records to the requested path."""


class FinalizerDependencies(typ.NamedTuple):
    """Inject monotonic time and the metrics sink at the finalizer boundary."""

    clock: MonotonicClock
    sink: MetricsSink


def classify_event(event_name: str) -> str:
    """Map one workflow event onto the fixed event vocabulary."""
    return event_name if event_name in EVENTS else "other"


def classify_decision(value: object) -> str:
    """Map a recorded scope decision onto its fixed vocabulary."""
    return value if isinstance(value, str) and value in DECISIONS else "unknown"


def _was_cancelled(inputs: WorkflowInputs) -> bool:
    """Match cancellation from any step observed by the finalizer."""
    statuses = (
        inputs.job_status,
        inputs.scope_outcome,
        inputs.cache_outcome,
        inputs.proofs_outcome,
        inputs.mutations_outcome,
    )
    return "cancelled" in statuses


def _scope_failed(inputs: WorkflowInputs) -> bool:
    """Match a failed scope decision step."""
    return inputs.scope_outcome == "failure"


def _was_explicitly_skipped(inputs: WorkflowInputs) -> bool:
    """Match the scope helper's exact false output."""
    return inputs.run_proofs == "false"


def _has_invalid_decision(inputs: WorkflowInputs) -> bool:
    """Match a run-proofs output outside its exact Boolean vocabulary."""
    return inputs.run_proofs not in {"true", "false"}


def _proof_failed(inputs: WorkflowInputs) -> bool:
    """Match a failed Kani harness step."""
    return inputs.proofs_outcome == "failure"


def _proofs_did_not_run_and_job_failed(inputs: WorkflowInputs) -> bool:
    """Match a job failure before the harness completed successfully."""
    return inputs.proofs_outcome != "success" and inputs.job_status == "failure"


def _mutation_failed(inputs: WorkflowInputs) -> bool:
    """Match a failed mutation compile gate."""
    return inputs.mutations_outcome == "failure"


def _proof_steps_succeeded(inputs: WorkflowInputs) -> bool:
    """Match successful harness and mutation steps."""
    return inputs.proofs_outcome == "success" and inputs.mutations_outcome == "success"


def _job_failed_after_proofs(inputs: WorkflowInputs) -> bool:
    """Match a later job failure after both proof steps succeeded."""
    return _proof_steps_succeeded(inputs) and inputs.job_status == "failure"


def _job_succeeded_after_proofs(inputs: WorkflowInputs) -> bool:
    """Match a successful job after both proof steps succeeded."""
    return _proof_steps_succeeded(inputs) and inputs.job_status == "success"


OUTCOME_RULES = (
    (_was_cancelled, "cancelled"),
    (_scope_failed, "scope_failure"),
    (_was_explicitly_skipped, "skipped"),
    (_has_invalid_decision, "invalid_decision"),
    (_proof_failed, "proof_failure"),
    (_proofs_did_not_run_and_job_failed, "proof_not_run"),
    (_mutation_failed, "mutation_failure"),
    (_job_failed_after_proofs, "job_failure"),
    (_job_succeeded_after_proofs, "success"),
)


def classify_outcome(inputs: WorkflowInputs) -> str:
    """Apply the documented precedence to scope, proof, mutation and job states."""
    return next(
        (outcome for predicate, outcome in OUTCOME_RULES if predicate(inputs)),
        "unknown",
    )


def classify_cache(cache_outcome: str, cache_hit: str) -> str:
    """Classify the restore action without inferring cache service health."""
    if cache_outcome == "skipped":
        return "not_attempted"
    if cache_outcome == "success":
        return "hit" if cache_hit == "true" else "miss"
    return "unknown"


def metrics_records(
    decision: object,
    inputs: WorkflowInputs,
    start_ns: int | None,
    end_ns: int | None,
) -> list[dict[str, object]]:
    """Build the run, cache and optional duration observations in fixed order."""
    event = classify_event(inputs.event_name)
    outcome = classify_outcome(inputs)
    records: list[dict[str, object]] = [
        {
            "name": "netsuke_kani_smoke_run_total",
            "labels": {
                "event": event,
                "decision": classify_decision(decision),
                "outcome": outcome,
            },
            "value": 1,
        },
        {
            "name": "netsuke_kani_smoke_cache_total",
            "labels": {
                "result": classify_cache(inputs.cache_outcome, inputs.cache_hit)
            },
            "value": 1,
        },
    ]
    if start_ns is not None and end_ns is not None:
        elapsed = max(end_ns - start_ns, 0) / 1_000_000_000
        if math.isfinite(elapsed):
            records.append({
                "name": "netsuke_kani_smoke_proof_duration_seconds",
                "labels": {"event": event, "outcome": outcome},
                "value": elapsed,
            })
    return records


def mark_start(
    state_directory: Path, clock: MonotonicClock = time.monotonic_ns
) -> bool:
    """Record the monotonic start of the guarded Kani proof command."""
    return _write_mark(state_directory / MARK_START_FILE, clock)


def mark_end(state_directory: Path, clock: MonotonicClock = time.monotonic_ns) -> bool:
    """Record the monotonic end of the guarded Kani proof command."""
    return _write_mark(state_directory / MARK_END_FILE, clock)


def _write_mark(mark_file: Path, clock: MonotonicClock) -> bool:
    """Write one timestamp and report whether the mark was retained."""
    try:
        mark_file.parent.mkdir(parents=True, exist_ok=True)
        mark_file.write_text(f"{clock()}\n", encoding="ascii")
    except OSError:
        print(WARNING_MARK_WRITE, file=sys.stderr)
        return False
    return True


def _read_mark(mark_file: Path) -> int | None:
    """Return a non-negative decimal timestamp, or `None` for a bad mark."""
    try:
        value = int(mark_file.read_text(encoding="ascii").strip())
    except OSError, UnicodeDecodeError, ValueError:
        return None
    return value if value >= 0 else None


def _read_decision(decision_file: Path) -> str:
    """Return the decision value from the separate JSON record, if valid."""
    try:
        record = json.loads(decision_file.read_text(encoding="utf-8"))
    except OSError, UnicodeDecodeError, json.JSONDecodeError:
        return "unknown"
    if not isinstance(record, dict):
        return "unknown"
    return classify_decision(record.get("decision"))


def _duration_marks(
    state_directory: Path,
    clock: MonotonicClock,
    end_mark_recorded: str,
    end_mark_outcome: str,
) -> tuple[int | None, int | None]:
    """Read proof marks, falling back only when the end step was skipped."""
    start = _read_mark(state_directory / MARK_START_FILE)
    end_file = state_directory / MARK_END_FILE
    if start is None:
        print(WARNING_DURATION, file=sys.stderr)
        return None, None
    try:
        end_file.stat()
    except FileNotFoundError:
        if end_mark_outcome == "skipped" and not end_mark_recorded:
            print(WARNING_END_FALLBACK, file=sys.stderr)
            return start, clock()
        print(WARNING_DURATION, file=sys.stderr)
        return None, None
    except OSError:
        print(WARNING_DURATION, file=sys.stderr)
        return None, None
    end = _read_mark(end_file)
    if end is None:
        print(WARNING_DURATION, file=sys.stderr)
        return None, None
    return start, end


def _atomic_write(metrics_file: Path, records: list[dict[str, object]]) -> None:
    """Replace the metrics artifact atomically with the complete JSONL payload."""
    metrics_file.parent.mkdir(parents=True, exist_ok=True)
    temporary = metrics_file.with_name(f".{metrics_file.name}.tmp")
    try:
        payload = "".join(
            json.dumps(record, separators=(",", ":"), allow_nan=False) + "\n"
            for record in records
        )
        temporary.write_text(payload, encoding="utf-8")
        temporary.replace(metrics_file)
    finally:
        with contextlib.suppress(OSError):
            temporary.unlink(missing_ok=True)


DEFAULT_FINALIZER_DEPENDENCIES = FinalizerDependencies(
    clock=time.monotonic_ns,
    sink=_atomic_write,
)


def finalize(
    inputs: WorkflowInputs,
    files: FinalizerFiles,
    dependencies: FinalizerDependencies = DEFAULT_FINALIZER_DEPENDENCIES,
) -> None:
    """Write bounded run metrics from workflow inputs and retained marks."""
    state_directory = Path(files.state_directory)
    if inputs.run_proofs == "true":
        start_ns, end_ns = _duration_marks(
            state_directory,
            dependencies.clock,
            inputs.end_mark_recorded,
            inputs.end_mark_outcome,
        )
    else:
        start_ns, end_ns = None, None
    records = metrics_records(
        _read_decision(Path(files.decision_file)),
        inputs,
        start_ns,
        end_ns,
    )
    dependencies.sink(Path(files.metrics_file), records)


@app.command(name="mark-start")
def mark_start_command(
    state_directory: typ.Annotated[str, Parameter(env_var="INPUT_STATE_DIRECTORY")],
) -> None:
    """Write the proof start mark without interfering with the proof step."""
    if not mark_start(Path(state_directory)):
        raise SystemExit(1)


@app.command(name="mark-end")
def mark_end_command(
    state_directory: typ.Annotated[str, Parameter(env_var="INPUT_STATE_DIRECTORY")],
) -> None:
    """Write the proof end mark without interfering with later job steps."""
    if not mark_end(Path(state_directory)):
        raise SystemExit(1)


@app.command(name="finalize")
def finalize_command(
    inputs: WorkflowInputs,
    files: FinalizerFiles,
) -> None:
    """Finalize one Kani job attempt from environment-backed inputs."""
    try:
        finalize(inputs, files)
    except OSError:
        print(ERROR_METRICS_WRITE, file=sys.stderr)
        raise SystemExit(1) from None


if __name__ == "__main__":
    app()
