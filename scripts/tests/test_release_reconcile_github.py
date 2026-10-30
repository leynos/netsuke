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

from hypothesis import given, settings
from hypothesis import strategies as st
from release_reconcile_github import (
    AssetAction,
    conflicts,
    plan_asset_actions,
)

#: Digests stand in for bare values after the GitHub adapter normalises the
#: provider's ``sha256:<hex>`` representation.
DIGEST_A = "a" * 64
DIGEST_B = "b" * 64

#: One SHA-256 digest, lowercase hexadecimal, generated rather than fixed.
#:
#: The alphabet is narrowed to hexadecimal because the value is a digest; the
#: *values* are not narrowed, so a generated pair covers both the matching and
#: the non-matching case from one draw.
_DIGEST = st.text(alphabet="0123456789abcdef", min_size=64, max_size=64)

#: An asset name. Constrained to the characters release assets actually use so
#: a failure names something a reader recognises; the interesting variation is
#: in the digests, not the file names.
_NAME = st.text(
    alphabet="abcdefghijklmnopqrstuvwxyz0123456789.-", min_size=1, max_size=12
)


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
        {"netsuke.tar.gz.sha256": DIGEST_B},
    )
    assert plan["netsuke.tar.gz"] is AssetAction.UPLOAD, (
        "an absent archive must be planned for upload"
    )
    assert plan["netsuke.tar.gz.sha256"] is AssetAction.SKIP_VERIFIED, (
        "a sidecar whose digest matches must be left alone"
    )


def test_a_present_asset_with_the_right_digest_is_skipped() -> None:
    """Require a verified asset to be left alone rather than re-uploaded."""
    plan = _plan({"netsuke.tar.gz": DIGEST_A}, {"netsuke.tar.gz": DIGEST_A})
    assert plan["netsuke.tar.gz"] is AssetAction.SKIP_VERIFIED, (
        "an asset whose published digest matches must not be re-uploaded"
    )


def test_a_present_asset_with_the_wrong_digest_conflicts() -> None:
    """Require different bytes under the same name to conflict.

    The name matches and the content does not. Overwriting is not available —
    GitHub assets are immutable once uploaded — so the only safe answer is to
    stop.
    """
    plan = _plan({"netsuke.tar.gz": DIGEST_A}, {"netsuke.tar.gz": DIGEST_B})
    assert plan["netsuke.tar.gz"] is AssetAction.CONFLICT, (
        "different bytes under the same name must stop the release"
    )
    assert conflicts(
        plan_asset_actions(
            expected={"netsuke.tar.gz": DIGEST_A},
            present={"netsuke.tar.gz": DIGEST_B},
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
        {"netsuke.tar.gz": DIGEST_A, "stowaway.txt": DIGEST_B},
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


def test_the_policy_does_not_interpret_github_digest_syntax() -> None:
    """Leave provider prefixes to the adapter that reads GitHub's response."""
    plan = _plan(
        {"netsuke.tar.gz": DIGEST_A},
        {"netsuke.tar.gz": f"sha256:{DIGEST_A}"},
    )
    assert plan["netsuke.tar.gz"] is AssetAction.CONFLICT, (
        "the pure policy must compare provider digest strings without rewriting them"
    )


def test_every_expected_asset_receives_exactly_one_action() -> None:
    """Require the plan to decide every expected asset exactly once.

    A missing entry would be silently treated as "nothing to do" by a caller
    iterating the result, which is how an asset never gets uploaded.
    """
    expected = {f"{n}.tar.gz": DIGEST_A for n in range(5)}
    present = {f"{n}.tar.gz": DIGEST_A for n in (1, 3)}
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
        present={"a.tar.gz": DIGEST_B, "b.tar.gz": DIGEST_B},
    )
    assert conflicts(actions) == ("a.tar.gz", "b.tar.gz"), (
        "every conflicting name must be reported, in plan order"
    )


@settings(max_examples=200, deadline=None, derandomize=True)
@given(published=_DIGEST, intended=_DIGEST)
def test_skipping_turns_on_digest_equality_alone(published: str, intended: str) -> None:
    """Require the skip decision to follow digest equality for any pair.

    This is the whole safety property of the module in one two-sided claim: an
    attached asset whose digest equals the expected one must be skipped, and
    one whose digest differs must conflict. Both directions are asserted in a
    single test on purpose. Split apart, a planner that always answered
    ``SKIP_VERIFIED`` would pass the matching half and one that always answered
    ``CONFLICT`` the other, so neither half alone is evidence that the
    comparison happens at all.

    The two digests are drawn independently, so Hypothesis reaches the equal
    and the unequal case from one property rather than from two handwritten
    constants. ``derandomize`` keeps a shrink reproducible.
    """
    action = _plan(
        {"netsuke.tar.gz": intended},
        {"netsuke.tar.gz": published},
    )["netsuke.tar.gz"]
    expected = (
        AssetAction.SKIP_VERIFIED
        if published.lower() == intended.lower()
        else AssetAction.CONFLICT
    )
    assert action is expected, (
        f"the decision must turn on digest equality alone: published "
        f"{published!r} against intended {intended!r} gave {action!r}"
    )


@settings(max_examples=200, deadline=None, derandomize=True)
@given(
    expected=st.dictionaries(_NAME, _DIGEST, min_size=1, max_size=8),
    # The same generator as ``expected`` so overlap is common, plus names drawn
    # again below, so the plan is exercised with a mix of absent, matching, and
    # extra assets in each example.
    attached=st.booleans(),
)
def test_every_expected_name_is_decided_once_and_in_order(
    expected: dict[str, str], *, attached: bool
) -> None:
    """Require the plan to be a total, single-entry, name-ordered decision.

    Totality is what a caller iterates: an expected name missing from the plan
    reads as "nothing to do", which is how an asset never gets uploaded. A
    duplicate entry would let a caller act twice on one name. Ordering makes
    the result a value two runs can be compared against.

    ``attached`` switches between an empty ``present`` and one agreeing with
    every expected name, so the same structural property covers both the
    upload and the skip branch rather than describing only one of them. The
    action itself is asserted too: with every asset absent the plan must be
    all uploads, and with every digest matching it must be all skips.
    """
    present = dict(expected) if attached else {}
    uniform = AssetAction.SKIP_VERIFIED if attached else AssetAction.UPLOAD

    plan = plan_asset_actions(expected=expected, present=present)

    names = [name for name, _ in plan]
    assert names == sorted(expected), (
        "the plan must decide every expected name, once, in name order"
    )
    assert len(names) == len(set(names)), "no asset name may be decided twice"
    assert {action for _, action in plan} == {uniform}, (
        f"every asset in this example must plan {uniform!r}"
    )


@settings(max_examples=200, deadline=None, derandomize=True)
@given(
    expected=st.dictionaries(_NAME, _DIGEST, min_size=1, max_size=5),
    undeclared=st.dictionaries(_NAME, st.one_of(st.none(), _DIGEST), max_size=5),
)
def test_undeclared_assets_never_enter_the_plan(
    expected: dict[str, str], undeclared: dict[str, str | None]
) -> None:
    """Require an attached asset the record does not declare to be ignored.

    An extra attached asset is the inventory check's finding, not something to
    reconcile here. Reporting it as ``UPLOAD`` would be false — it is present —
    and as ``CONFLICT`` would block a release over a name the plan never
    claimed. The two mappings are drawn independently, so they collide freely,
    which is what makes the ignored case and the decided case reachable from
    one example.
    """
    present = dict.fromkeys(expected) | undeclared

    plan = plan_asset_actions(expected=expected, present=present)

    assert [name for name, _ in plan] == sorted(expected), (
        "an undeclared attached asset must not appear in the plan"
    )
