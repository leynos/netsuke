"""Hold the per-call upload trace to one bounded record per ``gh`` call.

The aggregate timings answer "how long did listing take". They cannot answer
"which of the eleven calls failed", because a boundary crossed several times is
summed into a single number. These cases drive the uploader against the shared
``gh`` stub and read the file it left behind, so the claim under test is the
record a real run produces rather than a shape a test asserted on its own.

Boundedness is the other half. The trace is an artefact that outlives the run,
so a case here refuses anything beyond the fixed field set: no asset name, no
URL, and no command output may appear in a line it writes.

Run via ``make test-release-publication``.
"""

import json
import typing as typ

import pytest
import release_upload_trace
from release_upload_support import DEB, RPM, Exchange, Scenario, run_cli

if typ.TYPE_CHECKING:
    import pathlib

#: The fields every record carries, in addition to the run identity the
#: uploader reads from its environment.
FIXED_FIELDS = frozenset({
    "record_type",
    "event",
    "operation",
    "timestamp_ns",
    "outcome",
    "duration_seconds",
})

#: The run-identity keys the CLI supplies. Fixed so a query never discovers them.
CONTEXT_KEYS = frozenset({"run_id", "job", "run_attempt"})

#: The only error category a failed call may be attributed to.
ONLY_ERROR_CATEGORY = "call_failed"


def _scenario(tmp_path: pathlib.Path) -> Scenario:
    """Stage a two-asset payload, both of them absent from the release."""
    return Scenario(tmp_path, contents={DEB: b"deb bytes", RPM: b"rpm bytes"})


def _records(run: Scenario) -> list[dict[str, object]]:
    """Return the decoded trace records the run left beside its plan."""
    path = release_upload_trace.trace_path(run.plan)
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]


def test_every_call_is_traced_once(
    tmp_path: pathlib.Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Write one record per ``gh`` invocation, in the order they finished.

    A run that uploaded two declared assets makes four calls: the reconciling
    listing, the transfer, and the verifying re-listing cannot be told apart
    from four aggregate numbers, and only a per-call record can say the
    transfer is the one that failed.
    """
    scenario = _scenario(tmp_path)
    exchange = Exchange(attached={})
    assert run_cli(scenario, exchange, monkeypatch) == 0, "the upload must succeed"
    records = _records(scenario)
    assert [record["operation"] for record in records] == [
        "release_listing",
        "asset_upload",
        "release_verification",
    ], records
    assert [record["outcome"] for record in records] == ["completed"] * 3, records
    assert len(exchange.calls) == len(records), (exchange.calls, records)


def test_a_call_that_did_not_happen_is_not_traced(
    tmp_path: pathlib.Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Leave an untaken boundary absent when the release already matches.

    A release that holds every declared asset needs one call, not four. A trace
    that recorded every possible boundary would report a transfer that never
    happened, which is the failure the timings file already avoids by writing
    ``null`` rather than zero.
    """
    scenario = Scenario(tmp_path, contents={DEB: b"deb bytes"})
    # Annotated because the exchange records a digest *or* a `None` for an
    # asset whose bytes GitHub cannot vouch for, and `dict` is invariant in its
    # value type: a mapping of plain strings would not fit that wider shape.
    attached: dict[str, str | None] = {DEB: f"sha256:{scenario.digests()[DEB]}"}
    exchange = Exchange(attached=attached)
    assert run_cli(scenario, exchange, monkeypatch) == 0, "the upload must succeed"
    operations = [record["operation"] for record in _records(scenario)]
    assert operations == ["release_listing"], operations
    assert len(exchange.calls) == 1, exchange.calls


def test_a_refused_run_traces_the_call_that_refused(
    tmp_path: pathlib.Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Keep the trace of a run that refused: that is the run it matters for.

    The transfer is the call that failed: the listing before it completed, so
    the record names one of each, with the failure on the second rather than on
    the run as a whole.
    """
    scenario = _scenario(tmp_path)
    exchange = Exchange(attached={}, fail_upload=True)
    assert run_cli(scenario, exchange, monkeypatch) == 3, "the transfer must refuse"
    records = _records(scenario)
    assert [record["operation"] for record in records] == [
        "release_listing",
        "asset_upload",
    ], records
    assert records[0]["outcome"] == "completed", records
    failed = records[-1]
    assert failed["outcome"] == "failed", failed
    assert failed["error_category"] == ONLY_ERROR_CATEGORY, failed


def test_a_record_carries_no_field_outside_the_fixed_set(
    tmp_path: pathlib.Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Keep the artefact bounded: no asset name, URL, or command output.

    The trace outlives the run, and an asset name is both unbounded and
    attacker-visible in a way a duration is not. A completed record carries no
    error category at all, rather than a null one a reader would have to branch
    on differently from an absent key.
    """
    scenario = _scenario(tmp_path)
    assert run_cli(scenario, Exchange(attached={}), monkeypatch) == 0, (
        "the fixture must drive a completed run"
    )
    for record in _records(scenario):
        assert set(record) == FIXED_FIELDS | CONTEXT_KEYS, record
        assert record["record_type"] == "trace_event", record
        assert record["event"] == "release_upload_call", record
        assert isinstance(record["duration_seconds"], float), record
        assert isinstance(record["timestamp_ns"], int), record
        assert DEB not in json.dumps(record), record
        assert RPM not in json.dumps(record), record


def test_an_unknown_operation_is_refused() -> None:
    """Refuse an operation outside the bounded vocabulary at construction.

    A free-form operation name would make the trace unfilterable, and the
    refusal belongs where the mistake is made rather than at the reader.
    """
    with pytest.raises(ValueError, match="unknown upload trace operation"):
        release_upload_trace.TraceRecord(
            operation="gh_release_view",
            outcome=release_upload_trace.OUTCOME_COMPLETED,
            error_category=None,
            duration_seconds=0.0,
            timestamp_ns=0,
            context={},
        )


def test_an_unwritable_destination_is_reported_to_the_caller(
    tmp_path: pathlib.Path,
) -> None:
    """Surface the ``OSError`` rather than swallowing it and printing.

    The plan is the contract and the trace is a diagnostic, but that judgement
    belongs to the CLI boundary that can report it. A writer that swallowed the
    error could not tell its caller whether the record exists, and it would be
    deciding the run's outcome from inside a file write.
    """
    plan = tmp_path / "absent" / "upload-plan.json"
    with pytest.raises(OSError, match=r"upload-trace.jsonl"):
        release_upload_trace.record_trace(plan, release_upload_trace.CallTrace())


def test_the_trace_sits_beside_the_plan(tmp_path: pathlib.Path) -> None:
    """Name the trace in the plan's directory, so one retention keeps both."""
    plan = tmp_path / "records" / "upload-plan.json"
    destination = release_upload_trace.trace_path(plan)
    assert destination == tmp_path / "records" / "upload-trace.jsonl", destination


def test_the_context_carries_only_the_fixed_run_keys() -> None:
    """Supply the run identity from the caller, with an explicit absence.

    The seam that writes a record must not read ``os.environ`` itself: the
    command line owns process state, and a test supplies its own mapping. An
    environment with none of the three still yields the keys, so a consumer
    never has to distinguish a missing key from an absent value.
    """
    context = release_upload_trace.trace_context({})
    assert context == {
        "run_id": release_upload_trace.UNKNOWN,
        "job": release_upload_trace.UNKNOWN,
        "run_attempt": release_upload_trace.UNKNOWN,
    }, context
    assert set(context) == CONTEXT_KEYS, context
