"""Behavioural coverage for GitHub release asset reconciliation.

A re-run of a release must decide, per asset, whether to upload, leave alone,
or stop. The decision is driven by the digest GitHub reports for the asset
already present, which is the only evidence that the retained bytes are the
intended ones. Matching on names alone is what lets a release retain bytes
from an earlier run while claiming they are this run's.

The expectation is built from the live defect this work responds to: six
archives absent while their sidecars are present. That shape must plan six
uploads, not six skips.

Run via ``make test-release-publication``.
"""

import pytest
from release_reconcile_github import (
    DIGEST_PREFIX,
    AssetAction,
    conflicts,
    normalise_digest,
    plan_asset_actions,
)

#: Digests stand in for real ones. The prefix is the part that matters: GitHub
#: reports ``sha256:<hex>``, and an unnormalised comparison against a bare
#: hexadecimal digest would report a conflict for every asset that is correct.
DIGEST_A = "a" * 64
DIGEST_B = "b" * 64


def _plan(
    expected: dict[str, str], present: dict[str, str | None]
) -> dict[str, AssetAction]:
    """Return the plan as a mapping, for readable assertions."""
    return dict(plan_asset_actions(expected=expected, present=present))


def test_a_missing_asset_is_planned_for_upload() -> None:
    """Require an expected asset that is absent to be uploaded.

    This is the live defect: the sidecars published and the archives did not.
    A reconciler that treated "declared" as "present" would skip them.
    """
    plan = _plan(
        {"netsuke.tar.gz": DIGEST_A, "netsuke.tar.gz.sha256": DIGEST_B},
        {"netsuke.tar.gz.sha256": f"{DIGEST_PREFIX}{DIGEST_B}"},
    )
    assert plan["netsuke.tar.gz"] is AssetAction.UPLOAD, (
        "an absent archive must be planned for upload"
    )
    assert plan["netsuke.tar.gz.sha256"] is AssetAction.SKIP_VERIFIED, (
        "a sidecar whose digest matches must be left alone"
    )


def test_a_present_asset_with_the_right_digest_is_skipped() -> None:
    """Require a verified asset to be left alone rather than re-uploaded."""
    plan = _plan({"netsuke.tar.gz": DIGEST_A}, {"netsuke.tar.gz": f"sha256:{DIGEST_A}"})
    assert plan["netsuke.tar.gz"] is AssetAction.SKIP_VERIFIED, (
        "an asset whose published digest matches must not be re-uploaded"
    )


def test_a_present_asset_with_the_wrong_digest_conflicts() -> None:
    """Require different bytes under the same name to conflict.

    The name matches and the content does not. Overwriting is not available —
    GitHub assets are immutable once uploaded — so the only safe answer is to
    stop.
    """
    plan = _plan({"netsuke.tar.gz": DIGEST_A}, {"netsuke.tar.gz": f"sha256:{DIGEST_B}"})
    assert plan["netsuke.tar.gz"] is AssetAction.CONFLICT, (
        "different bytes under the same name must stop the release"
    )
    assert conflicts(
        plan_asset_actions(
            expected={"netsuke.tar.gz": DIGEST_A},
            present={"netsuke.tar.gz": f"sha256:{DIGEST_B}"},
        )
    ) == ("netsuke.tar.gz",), "the conflicting name must be reported"


def test_a_present_asset_with_no_reported_digest_conflicts() -> None:
    """Require an asset GitHub reports no digest for to conflict.

    ``None`` means the evidence is absent, not that it agrees. Treating an
    unverifiable asset as verified is the same failure as name-matching.
    """
    plan = _plan({"netsuke.tar.gz": DIGEST_A}, {"netsuke.tar.gz": None})
    assert plan["netsuke.tar.gz"] is AssetAction.CONFLICT, (
        "an asset with no reported digest has no evidence and must conflict"
    )


def test_undeclared_assets_are_not_decided_here() -> None:
    """Require only expected names to appear in the plan.

    An extra asset is the inventory check's business. Deciding it here would
    give two owners for one judgement, and the plan would then disagree with
    whichever check ran second.
    """
    plan = _plan(
        {"netsuke.tar.gz": DIGEST_A},
        {"netsuke.tar.gz": f"sha256:{DIGEST_A}", "stowaway.txt": f"sha256:{DIGEST_B}"},
    )
    assert set(plan) == {"netsuke.tar.gz"}, (
        "an undeclared asset must not appear in the plan"
    )


def test_the_plan_is_sorted_by_asset_name() -> None:
    """Require a stable order, so two runs of the same input read identically."""
    names = ["c.tar.gz", "a.tar.gz", "b.tar.gz"]
    plan = plan_asset_actions(expected=dict.fromkeys(names, DIGEST_A), present={})
    assert [name for name, _ in plan] == sorted(names), (
        "the plan must be ordered by asset name"
    )


@pytest.mark.parametrize(
    ("reported", "expected"),
    [
        (f"sha256:{DIGEST_A}", DIGEST_A),
        (f"sha256:{DIGEST_A.upper()}", DIGEST_A),
        (None, None),
        ("", None),
        ("sha256:", None),
        # A bare hexadecimal digest is NOT accepted. GitHub reports the
        # prefixed form, and `None` here means "no evidence", which
        # plan_asset_actions turns into a conflict. Passing the raw string
        # through instead would compare bare against prefixed and report a
        # conflict for a value we never read correctly.
        (DIGEST_A, None),
    ],
)
def test_normalise_digest_accepts_only_the_prefixed_form(
    reported: str | None, expected: str | None
) -> None:
    """Require the prefixed form to normalise and everything else to be ``None``.

    The prefixed and upper-case readings must compare equal to a bare digest,
    or a correct asset is reported as a conflict.
    """
    assert normalise_digest(reported) == expected, (
        "only GitHub's prefixed digest form may normalise to a bare digest"
    )


def test_every_expected_asset_receives_exactly_one_action() -> None:
    """Require the plan to decide every expected asset exactly once.

    A missing entry would be silently treated as "nothing to do" by a caller
    iterating the result, which is how an asset never gets uploaded.
    """
    expected = {f"{n}.tar.gz": DIGEST_A for n in range(5)}
    present = {f"{n}.tar.gz": f"sha256:{DIGEST_A}" for n in (1, 3)}
    plan = plan_asset_actions(expected=expected, present=present)
    assert [name for name, _ in plan] == sorted(expected), (
        "every expected asset must have its own entry"
    )
    assert len(plan) == len(expected), (
        "no expected asset may be decided twice or dropped"
    )


def test_an_empty_expectation_plans_nothing() -> None:
    """Pin the behaviour for an empty expectation as "plan nothing".

    Recorded rather than endorsed: this function is not the place that decides
    a release has assets, and an empty expectation is caught upstream by the
    inventory check, which compares the staged set against the declared one and
    refuses a declared set that is empty. Asserted here so the boundary is
    visible to whoever next reads this function and wonders why it does not
    guard the case itself.
    """
    assert not plan_asset_actions(expected={}, present={}), (
        "an empty expectation must produce an empty plan"
    )


def test_conflicts_reports_every_conflicting_name() -> None:
    """Require all conflicts to be reported, not only the first."""
    actions = plan_asset_actions(
        expected={"a.tar.gz": DIGEST_A, "b.tar.gz": DIGEST_A, "c.tar.gz": DIGEST_A},
        present={"a.tar.gz": f"sha256:{DIGEST_B}", "b.tar.gz": f"sha256:{DIGEST_B}"},
    )
    assert conflicts(actions) == ("a.tar.gz", "b.tar.gz"), (
        "every conflicting name must be reported, in plan order"
    )
