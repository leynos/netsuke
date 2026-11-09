"""Test the plan an upload run records and the request that drives it.

The plan is the run's audit trail: an operator reads it after a refusal to see
which names were going to move, and a job reads it to decide what to report.
These cases pin the serialized shape rather than the in-memory value, because
the shape is what crosses a job boundary.

Run via ``make test-release-publication``.
"""

import json
import pathlib
import typing as typ

import pytest
import release_reconcile_github
from release_reconcile_github import AssetAction
from release_upload_failures import DigestFailure
from release_upload_plan import (
    PLAN_NAME,
    Blockers,
    UploadRequest,
    blocked_message,
    conflict_message,
    unverifiable_message,
    write_plan,
)


class UploadPlan(typ.TypedDict):
    """Describe the decoded plan a run wrote.

    Attributes
    ----------
    actions
        One ``[name, action]`` pair per decided asset, in the order written.
    conflicts
        The names whose attached bytes were read and disagreed.
    unverifiable
        The names whose attached bytes could not be read at all.

    Notes
    -----
    Typed rather than left as ``object`` so a case reading ``["conflicts"]`` is
    checked against a mapping. A case still compares the whole plan against a
    literal, which is what pins the serialized shape, so the value types stay
    wide: the point of the comparison is the encoding, not the payload.
    """

    actions: list[list[str]]
    conflicts: list[str]
    unverifiable: list[str]
    unverifiable_reasons: dict[str, str]


def read_plan(path: pathlib.Path) -> UploadPlan:
    """Return the decoded plan at ``path``.

    Returns
    -------
    UploadPlan
        The decoded JSON, described by what the writer records.
    """
    decoded: UploadPlan = json.loads(path.read_text(encoding="utf-8"))
    return decoded


def test_the_plan_name_is_a_plain_file_name() -> None:
    """Keep the plan name free of separators so it survives a glob-free copy."""
    assert PLAN_NAME == "upload-plan.json", "the plan name must stay plain"


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
        Blockers(),
    )
    assert read_plan(destination) == {
        "actions": [["a.deb", "upload"], ["b.rpm", "skip-verified"]],
        "conflicts": [],
        "unverifiable": [],
        "unverifiable_reasons": {},
    }, "every action must be recorded in the order given"


def test_write_plan_records_the_conflicts_it_was_given(
    tmp_path: pathlib.Path,
) -> None:
    """Record the conflicting names so a failed run says which asset blocked it."""
    destination = tmp_path / PLAN_NAME
    write_plan(destination, [("a.deb", AssetAction.CONFLICT)], Blockers(("a.deb",)))
    assert read_plan(destination)["conflicts"] == ["a.deb"], (
        "the plan must name the blocked asset"
    )


def test_write_plan_is_newline_terminated(tmp_path: pathlib.Path) -> None:
    """Terminate the file so it appends cleanly to a job log."""
    destination = tmp_path / PLAN_NAME
    write_plan(destination, [], Blockers())
    assert destination.read_text(encoding="utf-8").endswith("}\n"), (
        "the plan must be newline-terminated"
    )


def test_two_identical_runs_serialize_identically(tmp_path: pathlib.Path) -> None:
    """Write byte-identical plans for identical decisions.

    A job that diffs two attempts' plans is comparing decisions, so key or
    ordering drift would read as a changed release.
    """
    actions = [("b.rpm", AssetAction.UPLOAD), ("a.deb", AssetAction.CONFLICT)]
    first = tmp_path / "first.json"
    second = tmp_path / "second.json"
    write_plan(first, actions, Blockers(("a.deb",)))
    write_plan(second, actions, Blockers(("a.deb",)))
    assert first.read_bytes() == second.read_bytes(), (
        "identical decisions must serialize identically"
    )


def test_a_conflicting_run_still_records_its_plan(tmp_path: pathlib.Path) -> None:
    """Record the plan a refusing run produced.

    An operator who sees only the conflict message still has to know which
    names were planned, and a plan written only on success would withhold it.
    """
    destination = tmp_path / PLAN_NAME
    write_plan(
        destination,
        [("a.deb", AssetAction.CONFLICT), ("b.rpm", AssetAction.UPLOAD)],
        Blockers(("a.deb",)),
    )
    plan = read_plan(destination)
    assert plan["actions"] == [["a.deb", "conflict"], ["b.rpm", "upload"]], (
        "a refused run must still record every planned action"
    )


def test_write_plan_reports_an_unwritable_destination(tmp_path: pathlib.Path) -> None:
    """Surface a write failure rather than reporting a plan that is not there."""
    with pytest.raises(OSError, match="absent"):
        write_plan(tmp_path / "absent" / PLAN_NAME, [], Blockers())


def test_conflict_message_names_every_conflict() -> None:
    """Name each conflicting asset in one line an operator can act on."""
    assert conflict_message(["a.deb", "b.rpm"]) == (
        "the release holds assets that disagree with the trusted record: a.deb, b.rpm"
    ), "every conflicting name must appear"


def test_conflict_message_leads_with_the_reason() -> None:
    """State why the run refused before listing what it refused."""
    assert conflict_message(["a.deb"]).startswith(
        "the release holds assets that disagree"
    ), "the reason must lead the message"


def test_unverifiable_message_does_not_claim_a_disagreement() -> None:
    """Describe an unread digest without asserting the bytes are wrong.

    Nothing compared them. Reporting a disagreement would promise evidence the
    run does not have and send an operator hunting for wrong bytes that may be
    correct.
    """
    message = unverifiable_message(["a.deb"])
    assert "could not be verified" in message, message
    assert "disagree" not in message, "an unread digest is not a disagreement"


def test_blocked_message_names_both_kinds_when_both_appear() -> None:
    """Report every reason that applies, not just the first found."""
    message = blocked_message(["a.deb"], ["b.rpm"])
    assert "disagree with the trusted record: a.deb" in message, message
    assert "could not be verified: b.rpm" in message, message


def test_blocked_message_omits_a_kind_with_no_names() -> None:
    """Withhold a clause about an absence that is not the finding."""
    assert blocked_message(["a.deb"], []) == conflict_message(["a.deb"]), (
        "a run with no unverifiable names must not mention them"
    )
    assert blocked_message([], ["b.rpm"]) == unverifiable_message(["b.rpm"]), (
        "a run with no conflicts must not mention them"
    )


def test_a_plan_records_the_two_kinds_of_blocker_apart(
    tmp_path: pathlib.Path,
) -> None:
    """Serialize conflicts and unverifiable names under their own keys."""
    destination = tmp_path / PLAN_NAME
    actions = release_reconcile_github.plan_asset_actions(
        expected={"a.deb": "aa", "b.rpm": "bb"},
        present={"a.deb": "zz", "b.rpm": None},
    )
    write_plan(destination, actions, Blockers.of(actions))
    plan = read_plan(destination)
    assert plan["conflicts"] == ["a.deb"], "the disagreement must be recorded"
    assert plan["unverifiable"] == ["b.rpm"], "the unread digest must be recorded"
    assert plan["actions"] == [["a.deb", "conflict"], ["b.rpm", "unverifiable"]], (
        "each name must carry the action that describes it"
    )


def test_an_upload_request_holds_the_whole_run() -> None:
    """Carry the tag, payload, record, plan, and mode in one value."""
    request = UploadRequest(
        tag="v1.0.0",
        payload_dir=pathlib.Path("payload"),
        digest_record=pathlib.Path("trusted.json"),
        selection_record=pathlib.Path("selection.json"),
        plan_path=pathlib.Path("plan.json"),
        is_dry_run=False,
    )
    assert (request.tag, request.is_dry_run) == ("v1.0.0", False), (
        "the request must carry the tag and the mode unchanged"
    )


@pytest.mark.parametrize(
    ("field", "replacement"),
    [("tag", "v2.0.0"), ("is_dry_run", False)],
)
def test_an_upload_request_is_immutable(field: str, replacement: object) -> None:
    """Refuse a mutation that would change what a run was asked to do.

    The field name is a parameter rather than a literal so the assignment is a
    store to a value the checker cannot resolve, which leaves the runtime
    refusal reachable: a typed store to a frozen field is rejected at analysis
    time, and the case would then test the checker instead of the dataclass.
    """
    request = UploadRequest(
        tag="v1.0.0",
        payload_dir=pathlib.Path("payload"),
        digest_record=pathlib.Path("trusted.json"),
        selection_record=pathlib.Path("selection.json"),
        plan_path=pathlib.Path("plan.json"),
        is_dry_run=True,
    )
    with pytest.raises(AttributeError):
        setattr(request, field, replacement)


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
    write_plan(destination, actions, Blockers.of(actions))
    recorded = read_plan(destination)
    expected = {
        "actions": [
            ["a.deb", "skip-verified"],
            ["b.rpm", "conflict"],
            ["c.pkg", "upload"],
        ],
        "conflicts": ["b.rpm"],
        "unverifiable": [],
        "unverifiable_reasons": {},
    }
    assert recorded == expected, "the plan must express every reconciliation action"


def test_the_plan_records_each_unverifiable_assets_bounded_reason(
    tmp_path: pathlib.Path,
) -> None:
    """Record why an unverifiable asset could not be verified, in bounded words.

    The action alone says the run could not tell; it does not say whether a
    retry is worth trying. The category does, and it is a fixed vocabulary
    rather than the failing command's own message: a plan is retained beside
    the release, so whatever is written there is published.
    """
    destination = tmp_path / PLAN_NAME
    actions = release_reconcile_github.plan_asset_actions(
        expected={"a.deb": "aa", "b.rpm": "bb"}, present={"a.deb": None, "b.rpm": None}
    )
    write_plan(
        destination,
        actions,
        Blockers.of(
            actions,
            {
                "a.deb": DigestFailure.DOWNLOAD_FAILED,
                "b.rpm": DigestFailure.NOT_A_REGULAR_FILE,
            },
        ),
    )
    reasons = read_plan(destination)["unverifiable_reasons"]
    assert reasons == {
        "a.deb": "download-failed",
        "b.rpm": "not-a-regular-file",
    }, "each unverifiable asset must carry the category that explains it"


def test_a_reason_is_never_written_for_an_asset_that_verified(
    tmp_path: pathlib.Path,
) -> None:
    """Keep the reasons about unestablished digests and nothing else.

    A caller that passed every delivered file would put reasons beside names
    that have digests, and a reader would then see two contradictory accounts
    of one asset in the same file.
    """
    destination = tmp_path / PLAN_NAME
    actions = release_reconcile_github.plan_asset_actions(
        expected={"a.deb": "aa"}, present={"a.deb": "aa"}
    )
    write_plan(
        destination,
        actions,
        Blockers.of(actions, {"a.deb": DigestFailure.MISSING_FILE}),
    )
    assert read_plan(destination)["unverifiable_reasons"] == {}, (
        "a verified asset has no failure to explain"
    )
