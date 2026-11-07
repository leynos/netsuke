"""Test the exit status the upload step reports for each outcome.

A workflow branches on the status, so the statuses are part of the contract
rather than a diagnostic detail: a conflict asks an operator to check which
commit produced the attached bytes, an unreadable record asks for the producer
job to be fixed, and neither is a transfer to retry. Each case here runs
``main`` over a real payload with the stubbed exchange from
:mod:`release_upload_support`, and asserts only what the workflow reads: the
status, whether a plan was recorded, and whether anything reached ``gh``.

Run via ``make test-release-publication``.
"""

import typing as typ

import pytest
import release_upload_plan
from release_upload_support import (
    DEB,
    Destinations,
    Exchange,
    Invocation,
    Scenario,
    run_cli,
)
from upload_declared_assets_cli import (
    EXIT_CONFLICT,
    EXIT_INPUT_UNREADABLE,
    EXIT_OK,
    EXIT_OUTPUT_UNWRITABLE,
    EXIT_UPLOAD_FAILED,
)

if typ.TYPE_CHECKING:
    import pathlib


@pytest.fixture
def scenario(tmp_path: pathlib.Path) -> Scenario:
    """Build a two-asset payload with its trusted record.

    Returns
    -------
    Scenario
        The payload, record, and the request that names them.
    """
    return Scenario(tmp_path, contents={DEB: b"deb bytes", "b.rpm": b"rpm bytes"})


def test_main_writes_a_plan_on_success(
    scenario: Scenario, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Record the plan so the job can report what it did."""
    status = run_cli(scenario, Exchange(attached={}), monkeypatch)
    assert status == EXIT_OK, "a completed transfer must exit zero"
    # The plan sorts by name, and "-" precedes "_", so the RPM leads.
    assert scenario.stored_plan()["actions"] == [
        ["b.rpm", "skip-verified"],
        [DEB, "skip-verified"],
    ], "a successful run must record every asset as verified"


def test_main_reports_a_conflict_with_its_own_status(
    scenario: Scenario, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Exit distinctly on a conflict so a workflow can retry or escalate."""
    exchange = Exchange(attached={DEB: "sha256:" + "ff" * 32})
    status = run_cli(scenario, exchange, monkeypatch)
    assert status == EXIT_CONFLICT, "a conflict must not report success"
    assert scenario.stored_plan()["conflicts"] == [DEB], (
        "the plan must name the asset that blocked the run"
    )


def test_a_conflict_still_records_the_plan_it_refused(
    scenario: Scenario, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Record the decisions a refusing run made, so its log says what it saw."""
    exchange = Exchange(attached={DEB: "sha256:" + "ff" * 32})
    assert run_cli(scenario, exchange, monkeypatch) == EXIT_CONFLICT, (
        "a refused run must still report the conflict status"
    )
    # The plan sorts by name, so "b.rpm" precedes the deb it depends on.
    assert scenario.stored_plan()["actions"] == [
        ["b.rpm", "upload"],
        [DEB, "conflict"],
    ], "a refused run must record the decisions it made"


def test_a_conflicting_run_uploads_nothing(
    scenario: Scenario, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Refuse the whole transfer, so the release never holds mixed bytes."""
    exchange = Exchange(attached={DEB: "sha256:" + "ff" * 32})
    _ = run_cli(scenario, exchange, monkeypatch)
    assert not exchange.uploads(), "a conflict must block the whole transfer"


def test_main_reports_an_absent_record_as_unreadable_input(
    scenario: Scenario, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Exit distinctly when the trusted record is missing."""
    exchange = Exchange(attached={})
    status = run_cli(
        scenario,
        exchange,
        monkeypatch,
        invocation=Invocation(
            destinations=Destinations(record=scenario.root / "absent.json")
        ),
    )
    assert status == EXIT_INPUT_UNREADABLE, "an absent record must be input trouble"
    assert not exchange.calls, "an unreadable record must not reach the network"


def test_a_record_declaring_no_asset_is_unreadable_input(
    tmp_path: pathlib.Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Refuse an empty trusted record before anything reaches the network.

    A record with no assets would reconcile nothing and report success, which
    is the worst outcome available: a release that published nothing and said
    it was done.
    """
    scenario = Scenario(tmp_path, contents={DEB: b"deb bytes"})
    _ = scenario.record.write_text("{}", encoding="utf-8")
    exchange = Exchange(attached={})
    assert run_cli(scenario, exchange, monkeypatch) == EXIT_INPUT_UNREADABLE, (
        "a record with no asset must be refused as input trouble"
    )
    assert not exchange.calls, "an empty record must not reach the network"


def test_a_payload_that_disagrees_with_the_record_is_a_conflict(
    scenario: Scenario, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Report replaced bytes as a disagreement, not as a transfer failure."""
    _ = (scenario.payload / DEB).write_bytes(b"rebuilt")
    exchange = Exchange(attached={})
    assert run_cli(scenario, exchange, monkeypatch) == EXIT_CONFLICT, (
        "replaced bytes must report as a conflict"
    )
    assert not exchange.calls, "a conflicting payload must not reach the network"


def test_main_reports_an_unwritable_plan(
    scenario: Scenario, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Exit distinctly when the plan cannot be recorded."""
    status = run_cli(
        scenario,
        Exchange(attached={}),
        monkeypatch,
        invocation=Invocation(
            destinations=Destinations(
                plan=scenario.root / "absent" / release_upload_plan.PLAN_NAME
            )
        ),
    )
    assert status == EXIT_OUTPUT_UNWRITABLE, (
        "an unwritable plan must report output trouble, not success"
    )


def test_main_reports_a_transport_failure_with_its_own_status(
    scenario: Scenario, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Exit distinctly when the transfer fails, so a retry can be automated."""
    exchange = Exchange(attached={}, fail_upload=True)
    assert run_cli(scenario, exchange, monkeypatch) == EXIT_UPLOAD_FAILED, (
        "a failed transfer must report its own status"
    )


def test_main_reports_an_asset_absent_after_upload_as_a_failed_upload(
    scenario: Scenario, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Refuse to call the release complete when a declared asset never arrived.

    ``gh release upload`` can report success while the asset does not appear on
    the release. The re-listed plan then still asks to upload that name, which
    is not a conflict, so without an explicit check the step would exit zero
    over a release that is missing an asset.
    """
    exchange = Exchange(attached={}, drop_uploads=True)
    assert run_cli(scenario, exchange, monkeypatch) == EXIT_UPLOAD_FAILED, (
        "an asset still absent after the upload must not report success"
    )
    assert scenario.stored_plan()["actions"] == [
        ["b.rpm", "upload"],
        [DEB, "upload"],
    ], "the plan must record that the assets are still outstanding"


def test_a_dry_run_still_succeeds_with_every_asset_planned_for_upload(
    scenario: Scenario, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Leave a dry run's planned uploads alone: sending nothing is the point."""
    exchange = Exchange(attached={})
    status = run_cli(
        scenario, exchange, monkeypatch, invocation=Invocation(dry_run=True)
    )
    assert status == EXIT_OK, "a dry run must not report an unattempted upload"
    assert scenario.stored_plan()["actions"] == [
        ["b.rpm", "upload"],
        [DEB, "upload"],
    ], "a dry run's plan must still name the assets it would send"


def test_main_reports_success_only_when_the_release_holds_the_bytes(
    scenario: Scenario, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Exit zero on a re-run over a release this step already completed.

    Idempotence is the property that makes an interrupted job safe to restart,
    so a second run must reach the same status as the first.
    """
    exchange = Exchange(attached={})
    assert run_cli(scenario, exchange, monkeypatch) == EXIT_OK, (
        "the first run must complete successfully"
    )
    assert run_cli(scenario, exchange, monkeypatch) == EXIT_OK, (
        "a re-run over a completed release must also succeed"
    )
