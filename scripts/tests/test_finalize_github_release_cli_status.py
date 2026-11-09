"""Pin the finalizer's exit table against the planner's own action vocabulary.

The table is the whole contract between the finalizer and the workflow step
that reads its status: a run reaches `EXIT_OK` only when every attached asset
was verified, and every other action is a refusal of some kind. Two mistakes
matter, and neither needs a staged tree to see. A new action added to
`AssetAction` that the table does not name would fall to a default. An action
mapped to zero would let an unverified release be un-drafted, and un-drafting
cannot be undone.

A third mistake is about the inputs rather than the table: the workflow hands
the planner one identity record, one asset listing, and one output path, and a
planner that guessed a missing one would read something the run never staged.
That requirement is pinned here because it is part of the same step contract.

The remainder of the CLI's behaviour -- the plan it writes, the diagnoses it
prints, and the records it refuses -- is exercised in
`test_finalize_github_release_cli.py`.

Run via ``make test-release-publication``.
"""

import pytest
import release_reconcile_github
from finalize_github_release_cli import (
    ACTION_EXITS,
    EXIT_CONFLICT,
    EXIT_OK,
    EXIT_UPLOAD_REQUIRED,
    build_parser,
)


def test_the_exit_table_covers_every_action_the_planner_declares() -> None:
    """Require the table to name every action, so none falls to a default."""
    declared = {action.value for action in release_reconcile_github.AssetAction}
    assert set(ACTION_EXITS) == declared, (
        f"the exit table must cover exactly the planner's actions; "
        f"missing {sorted(declared - set(ACTION_EXITS))!r}, "
        f"extra {sorted(set(ACTION_EXITS) - declared)!r}"
    )


def test_the_exit_table_treats_only_a_verified_asset_as_success() -> None:
    """Require both other actions to stop the release before it is un-drafted."""
    successes = sorted(
        action for action, code in ACTION_EXITS.items() if code == EXIT_OK
    )
    assert successes == ["skip-verified"], (
        f"only a verified asset may proceed to un-drafting, found {successes!r}"
    )


def test_an_unverifiable_asset_stops_the_run_like_a_conflict_does() -> None:
    """Map the unreadable-bytes action to the conflict status.

    The plan keeps the two apart, and the status deliberately does not: since
    nothing may be published over bytes no one could read, a caller branching
    on the status alone must not read "unverifiable" as permission to proceed.
    """
    assert ACTION_EXITS["unverifiable"] == EXIT_CONFLICT, (
        "an asset whose bytes could not be established must stop the run"
    )


def test_the_conflict_code_is_distinct_from_the_upload_code() -> None:
    """Keep the two refusals apart, since they send an operator elsewhere."""
    assert EXIT_CONFLICT != EXIT_UPLOAD_REQUIRED, (
        "a conflict and a missing asset are different failures and must not "
        "share an exit code"
    )


@pytest.mark.parametrize("missing", ["--identity", "--assets", "--output"])
def test_the_cli_requires_all_three_files(missing: str) -> None:
    """Refuse to guess any of them, since each names a different input."""
    complete = {
        "--identity": "identity.json",
        "--assets": "assets.json",
        "--output": "plan.json",
    }
    argv = [
        part
        for flag, value in complete.items()
        if flag != missing
        for part in (flag, value)
    ]

    with pytest.raises(SystemExit):
        _ = build_parser().parse_args(argv)
