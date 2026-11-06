"""Test the plan an upload run records and the request that drives it.

The plan is the run's audit trail: an operator reads it after a refusal to see
which names were going to move, and a job reads it to decide what to report.
These cases pin the serialized shape rather than the in-memory value, because
the shape is what crosses a job boundary.

Run via ``make test-release-publication``.
"""

import json
import pathlib

import pytest
import release_reconcile_github
from release_reconcile_github import AssetAction
from release_upload_plan import PLAN_NAME, UploadRequest, conflict_message, write_plan


def read_plan(path: pathlib.Path) -> object:
    """Return the decoded plan at ``path``.

    Returns
    -------
    object
        The decoded JSON, left unchecked so a case can assert its shape.
    """
    return json.loads(path.read_text(encoding="utf-8"))


def test_the_plan_name_is_a_plain_file_name() -> None:
    """Keep the plan name free of separators so it survives a glob-free copy."""
    assert PLAN_NAME == "upload-plan.json"


def test_write_plan_records_every_action_in_name_order(
    tmp_path: pathlib.Path,
) -> None:
    """Record one pair per declared asset, in the order given."""
    destination = tmp_path / PLAN_NAME
    write_plan(
        destination,
        [
            ("a.deb", AssetAction.UPLOAD),
            ("b.rpm", AssetAction.SKIP_VERIFIED),
        ],
        [],
    )
    assert read_plan(destination) == {
        "actions": [["a.deb", "upload"], ["b.rpm", "skip-verified"]],
        "conflicts": [],
    }


def test_write_plan_records_the_conflicts_it_was_given(
    tmp_path: pathlib.Path,
) -> None:
    """Record the conflicting names so a failed run says which asset blocked it."""
    destination = tmp_path / PLAN_NAME
    write_plan(destination, [("a.deb", AssetAction.CONFLICT)], ["a.deb"])
    assert read_plan(destination)["conflicts"] == ["a.deb"]


def test_write_plan_is_newline_terminated(tmp_path: pathlib.Path) -> None:
    """Terminate the file so it appends cleanly to a job log."""
    destination = tmp_path / PLAN_NAME
    write_plan(destination, [], [])
    assert destination.read_text(encoding="utf-8").endswith("}\n")


def test_two_identical_runs_serialize_identically(tmp_path: pathlib.Path) -> None:
    """Write byte-identical plans for identical decisions.

    A job that diffs two attempts' plans is comparing decisions, so key or
    ordering drift would read as a changed release.
    """
    actions = [("b.rpm", AssetAction.UPLOAD), ("a.deb", AssetAction.CONFLICT)]
    first = tmp_path / "first.json"
    second = tmp_path / "second.json"
    write_plan(first, actions, ["a.deb"])
    write_plan(second, actions, ["a.deb"])
    assert first.read_bytes() == second.read_bytes()


def test_a_conflicting_run_still_records_its_plan(tmp_path: pathlib.Path) -> None:
    """Record the plan a refusing run produced.

    An operator who sees only the conflict message still has to know which
    names were planned, and a plan written only on success would withhold it.
    """
    destination = tmp_path / PLAN_NAME
    write_plan(
        destination,
        [("a.deb", AssetAction.CONFLICT), ("b.rpm", AssetAction.UPLOAD)],
        ["a.deb"],
    )
    plan = read_plan(destination)
    assert plan["actions"] == [["a.deb", "conflict"], ["b.rpm", "upload"]]


def test_write_plan_reports_an_unwritable_destination(tmp_path: pathlib.Path) -> None:
    """Surface a write failure rather than reporting a plan that is not there."""
    with pytest.raises(OSError, match="absent"):
        write_plan(tmp_path / "absent" / PLAN_NAME, [], [])


def test_conflict_message_names_every_conflict() -> None:
    """Name each conflicting asset in one line an operator can act on."""
    assert conflict_message(["a.deb", "b.rpm"]) == (
        "the release holds assets that disagree with the trusted record: a.deb, b.rpm"
    )


def test_conflict_message_leads_with_the_reason() -> None:
    """State why the run refused before listing what it refused."""
    assert conflict_message(["a.deb"]).startswith(
        "the release holds assets that disagree"
    )


def test_an_upload_request_holds_the_whole_run() -> None:
    """Carry the tag, payload, record, plan, and mode in one value."""
    request = UploadRequest(
        tag="v1.0.0",
        payload_dir=pathlib.Path("payload"),
        digest_record=pathlib.Path("trusted.json"),
        plan_path=pathlib.Path("plan.json"),
        is_dry_run=False,
    )
    assert (request.tag, request.is_dry_run) == ("v1.0.0", False)


def test_an_upload_request_is_immutable() -> None:
    """Refuse a mutation that would change what a run was asked to do."""
    request = UploadRequest(
        tag="v1.0.0",
        payload_dir=pathlib.Path("payload"),
        digest_record=pathlib.Path("trusted.json"),
        plan_path=pathlib.Path("plan.json"),
        is_dry_run=True,
    )
    with pytest.raises(AttributeError):
        request.tag = "v2.0.0"


def test_the_plan_round_trips_through_the_reconciliation_vocabulary(
    tmp_path: pathlib.Path,
) -> None:
    """Serialize every action the reconciliation can produce.

    The plan's values are the reconciliation's own, so a new action that the
    plan could not express would be a release decision with no audit trail.
    """
    actions = release_reconcile_github.plan_asset_actions(
        expected={"a.deb": "aa", "b.rpm": "bb", "c.pkg": "cc"},
        present={"a.deb": "aa", "b.rpm": "zz"},
    )
    destination = tmp_path / PLAN_NAME
    write_plan(destination, actions, release_reconcile_github.conflicts(actions))
    recorded = read_plan(destination)
    assert recorded == {
        "actions": [
            ["a.deb", "skip-verified"],
            ["b.rpm", "conflict"],
            ["c.pkg", "upload"],
        ],
        "conflicts": ["b.rpm"],
    }
