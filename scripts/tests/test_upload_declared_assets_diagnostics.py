"""Hold the uploader's two diagnostic records to their own fate at the CLI edge.

A run leaves three files beside its plan: the plan itself, the aggregate
timings, and the per-call trace. The plan is the contract -- a run that cannot
write it fails -- but the other two are diagnostics for the operator, and a
full disk must not turn a successful upload into a failed one.

That judgement lives at the CLI boundary, because only there is the plan's
priority known. The cases here prove the three halves of it: a writer surfaces
its ``OSError`` to whoever asked, the boundary reports each unwritable file
separately rather than swallowing it, and one failed write does not suppress
the other. They also check the outcome and operation names a real run traces,
since a trace whose outcomes never said ``failed`` would be an artefact no
operator could act on.

The upload decision itself is ``test_upload_declared_assets_cli``'s; this
module varies only how the diagnostics land.

Run via ``make test-release-publication``.
"""

import json
import typing as typ

import pytest
import release_upload_timing
import release_upload_timing_files
import release_upload_trace
from release_upload_support import DEB, RPM, Exchange, Scenario, run_cli

if typ.TYPE_CHECKING:
    import pathlib


@pytest.fixture
def scenario(tmp_path: pathlib.Path) -> Scenario:
    """Build a two-asset payload whose release holds nothing.

    Returns
    -------
    Scenario
        The payload, trusted records, and plan destination.
    """
    return Scenario(tmp_path, contents={DEB: b"deb bytes", RPM: b"rpm bytes"})


def _unwritable(monkeypatch: pytest.MonkeyPatch, module: object, writer: str) -> None:
    """Replace ``writer`` on ``module`` with one that raises ``OSError``.

    A real unwritable destination is a read-only filesystem or a full disk,
    neither of which a test can rely on. Patching the writer is the narrower
    claim anyway: what is under test is the boundary's reaction to an
    ``OSError``, not the filesystem's willingness to produce one.

    The patch goes through ``monkeypatch`` so it is undone at the end of the
    case. A bare ``setattr`` would leave every later case in the session
    writing to a writer that always fails, which would make an assertion about
    a written record pass or fail depending on test order.
    """

    def _refuse(*_args: object, **_keywords: object) -> typ.NoReturn:
        """Fail as an unwritable destination does."""
        msg = "no space left on device"
        raise OSError(msg)

    monkeypatch.setattr(module, writer, _refuse)


def test_a_completed_run_leaves_both_diagnostic_records(
    scenario: Scenario, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Leave a timings file and a trace beside the plan on a clean run.

    A record an operator can only obtain from a failing run is worthless, so
    the successful run has to produce them too.
    """
    assert run_cli(scenario, Exchange(attached={}), monkeypatch) == 0, "must succeed"
    timings = release_upload_timing_files.timings_path(scenario.plan)
    trace = release_upload_trace.trace_path(scenario.plan)
    assert release_upload_timing_files.read_timings(timings).listing_ms is not None, (
        "the reconciliation listing must be timed"
    )
    operations = [
        json.loads(line)["operation"] for line in trace.read_text().splitlines()
    ]
    assert operations, "the trace must record the calls the run made"
    assert trace.read_text(encoding="utf-8").strip(), "the trace must not be empty"


def test_an_unwritable_timings_file_does_not_fail_the_upload(
    scenario: Scenario, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Keep the exit status the upload earned when a diagnostic cannot land.

    The plan was written and every declared asset is attached, so the run
    succeeded. Reporting failure because a timing number could not be saved
    would make an operator retry a completed upload.
    """
    _unwritable(monkeypatch, release_upload_timing_files, "record_timings")

    assert run_cli(scenario, Exchange(attached={}), monkeypatch) == 0, (
        "a diagnostic failure must not change the upload's status"
    )


def test_an_unwritable_trace_does_not_fail_the_upload(
    scenario: Scenario, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Treat the trace's fate independently of the timings'."""
    _unwritable(monkeypatch, release_upload_trace, "record_trace")

    assert run_cli(scenario, Exchange(attached={}), monkeypatch) == 0, (
        "an unwritable trace must not fail the upload"
    )


def test_an_unwritable_diagnostic_is_reported_with_its_path(
    scenario: Scenario,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Name the file that could not be written, on standard error.

    A message that said only "measurements not recorded" would leave an
    operator unable to tell which of the two to look for.
    """
    _unwritable(monkeypatch, release_upload_timing_files, "record_timings")
    _ = run_cli(scenario, Exchange(attached={}), monkeypatch)

    reported = capsys.readouterr().err
    assert "upload timings not recorded" in reported, reported
    assert release_upload_timing.TIMINGS_NAME in reported, reported


def test_one_unwritable_diagnostic_does_not_suppress_the_other(
    scenario: Scenario, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Attempt both writes even when the first one fails.

    A shared ``try`` would tie the trace's fate to the timings', and the two
    are unrelated: a read-only timings file says nothing about whether the
    trace can be written.
    """
    _unwritable(monkeypatch, release_upload_timing_files, "record_timings")
    _ = run_cli(scenario, Exchange(attached={}), monkeypatch)

    trace = release_upload_trace.trace_path(scenario.plan)
    assert trace.read_text(encoding="utf-8").strip(), (
        "the trace must still be attempted after the timings write failed"
    )


def test_a_failing_trace_does_not_suppress_the_timings(
    scenario: Scenario, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Attempt the timings write first, and keep it when the trace fails."""
    _unwritable(monkeypatch, release_upload_trace, "record_trace")
    _ = run_cli(scenario, Exchange(attached={}), monkeypatch)

    timings = release_upload_timing_files.timings_path(scenario.plan)
    assert release_upload_timing_files.read_timings(timings).listing_ms is not None, (
        "the timings must still be written when the trace fails"
    )


def test_a_refused_run_still_records_its_diagnostics(
    scenario: Scenario, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Write both records for a run that refused, which most needs them.

    The refusal path returns before the plan is written, so a trace that were
    only recorded on the success path would be absent from exactly the run an
    operator is investigating.
    """
    assert (
        run_cli(scenario, Exchange(attached={}, fail_upload=True), monkeypatch) == 3
    ), "the transfer must refuse"

    timings = release_upload_timing_files.timings_path(scenario.plan)
    trace = release_upload_trace.trace_path(scenario.plan)
    assert release_upload_timing_files.read_timings(timings).upload_ms is not None, (
        "the failed transfer must still be timed"
    )
    outcomes = [json.loads(line)["outcome"] for line in trace.read_text().splitlines()]
    assert outcomes[-1] == "failed", outcomes


def test_the_reader_refuses_a_timings_file_a_writer_could_not_produce() -> None:
    """Keep the two boundaries consistent about what ``None`` means.

    The reader is what an operator's tooling uses to decide "the download never
    happened". If it silently accepted a zero where the writer writes ``null``,
    a run that downloaded nothing would read as an instantaneous download.
    """
    assert release_upload_timing.BoundaryTimings().download_ms is None, (
        "an uncrossed boundary must read as absent, not as zero"
    )
