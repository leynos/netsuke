"""Specify the decisions a release finalization run makes from recorded facts."""

import pytest
from finalize_github_release_plan import (
    FinalizePolicyError,
    PlanVerdict,
    assess_plan,
)
from finalize_github_release_state import (
    Readback,
    ReleaseState,
    classify_readback,
    is_already_finalized,
    parse_release_state,
)

ARCHIVE = "a.tar.gz"
SIDECAR = "a.tar.gz.sha256"


@pytest.mark.parametrize(
    ("plan", "verdict"),
    [
        pytest.param(
            {"actions": [[ARCHIVE, "skip-verified"]]},
            PlanVerdict.VERIFIED,
            id="every-asset-verified",
        ),
        pytest.param(
            {"actions": [[ARCHIVE, "upload"]]},
            PlanVerdict.UPLOAD_REQUIRED,
            id="asset-missing",
        ),
        pytest.param(
            {"actions": [[ARCHIVE, "conflict"]]},
            PlanVerdict.REFUSED,
            id="asset-disagrees",
        ),
        pytest.param(
            {"actions": [[ARCHIVE, "unverifiable"]]},
            PlanVerdict.REFUSED,
            id="digest-unreadable",
        ),
        pytest.param(
            {"actions": []},
            PlanVerdict.REFUSED,
            id="no-decisions-at-all",
        ),
        pytest.param(
            {"assets": {}},
            PlanVerdict.UNREADABLE,
            id="not-a-plan",
        ),
    ],
)
def test_the_plan_verdict_names_what_publication_may_do(
    plan: dict[str, object], verdict: PlanVerdict
) -> None:
    """Classify each plan the planner can produce."""
    assert assess_plan(plan).verdict is verdict, (
        f"the plan {plan!r} must be classified as {verdict}"
    )


def test_a_conflict_outranks_an_upload_in_the_same_plan() -> None:
    """Report the irreversible finding first, whatever the row order.

    The planner's own precedence puts a conflict ahead of a missing asset,
    and this stage must not invert it: uploading into a release that holds
    demonstrably wrong bytes would mix two runs' output.
    """
    plan = {
        "actions": [[ARCHIVE, "upload"], [SIDECAR, "conflict"]],
        "conflicts": [SIDECAR],
    }

    assessment = assess_plan(plan)

    assert assessment.verdict is PlanVerdict.REFUSED, (
        "a plan holding a conflict must be refused"
    )
    assert assessment.conflicts == (SIDECAR,), (
        "the conflict must be the finding that decides the verdict"
    )
    assert not assessment.uploads, (
        "a refused plan does not also advertise uploads as the next step"
    )


def test_the_top_level_summary_and_the_rows_are_both_honoured() -> None:
    """Union the two accounts rather than trusting either alone.

    A producer that writes only the summary and one that writes only rows
    both describe a release this stage must refuse; reading one surface would
    let the other hide a blocking asset.
    """
    plan = {
        "actions": [[ARCHIVE, "conflict"]],
        "conflicts": [SIDECAR],
    }

    assessment = assess_plan(plan)

    assert set(assessment.conflicts) == {ARCHIVE, SIDECAR}, (
        "every conflicting name must be reported, from either surface"
    )
    assert len(assessment.conflicts) == 2, "a name found twice must be reported once"


def test_unverifiable_assets_stay_distinct_from_conflicts() -> None:
    """Keep the two diagnoses apart, because the operator's next step differs."""
    plan = {
        "actions": [[ARCHIVE, "unverifiable"]],
        "conflicts": [SIDECAR],
    }

    assessment = assess_plan(plan)

    assert assessment.conflicts == (SIDECAR,), (
        "the summary's conflict must survive as a conflict"
    )
    assert assessment.unverifiable == (ARCHIVE,), (
        "an unreadable digest must not be reported as a conflict"
    )


@pytest.mark.parametrize(
    "plan",
    [
        pytest.param([], id="not-an-object"),
        pytest.param({}, id="no-actions-member"),
        pytest.param({"actions": "skip-verified"}, id="actions-not-a-list"),
        pytest.param({"actions": [[ARCHIVE]]}, id="row-too-short"),
        pytest.param({"actions": ["ab"]}, id="row-not-a-pair"),
        pytest.param({"actions": [["", "upload"]]}, id="row-without-a-name"),
        pytest.param({"actions": [[7, "upload"]]}, id="row-name-not-text"),
        pytest.param({"actions": [[ARCHIVE, 7]]}, id="row-action-not-text"),
        pytest.param({"actions": [[ARCHIVE, "clobber"]]}, id="unknown-action"),
        pytest.param({"actions": [], "conflicts": "a"}, id="summary-not-a-list"),
        pytest.param({"actions": [], "conflicts": [7]}, id="summary-name-not-text"),
    ],
)
def test_a_malformed_plan_is_unreadable_rather_than_verified(plan: object) -> None:
    """Refuse a document whose shape cannot support any decision."""
    assessment = assess_plan(plan)

    assert assessment.verdict is PlanVerdict.UNREADABLE, (
        "a plan whose shape supports no decision must be unreadable"
    )
    assert assessment.detail, "an unreadable plan must say why it was refused"


@pytest.mark.parametrize(
    ("plan", "diagnosis"),
    [
        pytest.param(
            {"actions": [[ARCHIVE]]},
            "not a name and action pair",
            id="row-too-short",
        ),
        pytest.param(
            {"actions": ["ab"]},
            "not a name and action pair",
            id="row-not-a-pair",
        ),
        pytest.param(
            {"actions": [[7, "upload"]]},
            "does not name an asset",
            id="row-name-not-text",
        ),
        pytest.param(
            {"actions": [["", "upload"]]},
            "does not name an asset",
            id="row-without-a-name",
        ),
        pytest.param(
            {"actions": [[ARCHIVE, 7]]},
            "carries an unknown action",
            id="row-action-not-text",
        ),
        pytest.param(
            {"actions": [[ARCHIVE, "clobber"]]},
            "carries an unknown action",
            id="unknown-action",
        ),
    ],
)
def test_each_malformed_row_reports_its_own_diagnosis(
    plan: object, diagnosis: str
) -> None:
    """Distinguish a malformed row's fault from the other rows' faults."""
    detail = assess_plan(plan).detail or ""

    assert diagnosis in detail, (
        f"the plan row's fault must be reported as {diagnosis!r}, got {detail!r}"
    )
    assert "plan row" in detail, "the diagnosis must name the row it refused"


def _payload(**overrides: object) -> dict[str, object]:
    """Return a well-formed release response with the given fields replaced."""
    payload: dict[str, object] = {
        "tagName": "v1.2.3",
        "isDraft": True,
        "isPrerelease": False,
        "assets": [{"name": ARCHIVE, "digest": "sha256:" + "ab" * 32}],
    }
    payload.update(overrides)
    return payload


def test_a_release_response_is_decoded_into_its_flags_and_digests() -> None:
    """Read the draft flag, prerelease flag, and reported digest together."""
    state = parse_release_state(_payload(), tag="v1.2.3")

    assert state == ReleaseState(
        tag="v1.2.3",
        is_draft=True,
        is_prerelease=False,
        assets={ARCHIVE: "sha256:" + "ab" * 32},
    ), "the response must decode into exactly its flags, tag, and digests"


def test_an_asset_without_a_usable_digest_is_preserved_as_unknown() -> None:
    """Do not turn an absent digest into an empty one that could compare."""
    payload = _payload(
        assets=[
            {"name": ARCHIVE},
            {"name": SIDECAR, "digest": 7},
        ]
    )

    state = parse_release_state(payload, tag="v1.2.3")

    assert state.assets == {ARCHIVE: None, SIDECAR: None}, (
        "an absent or non-text digest must be recorded as unknown, not empty"
    )


@pytest.mark.parametrize(
    "payload",
    [
        pytest.param([], id="not-an-object"),
        pytest.param(_payload(tagName="v9.9.9"), id="wrong-tag"),
        pytest.param(_payload(isDraft="yes"), id="draft-not-a-bool"),
        pytest.param(
            {"tagName": "v1.2.3", "isDraft": True, "assets": []},
            id="no-prerelease",
        ),
        pytest.param(_payload(assets={}), id="assets-not-a-list"),
        pytest.param(_payload(assets=["a"]), id="asset-not-an-object"),
        pytest.param(_payload(assets=[{}]), id="asset-unnamed"),
        pytest.param(
            _payload(assets=[{"name": ARCHIVE}, {"name": ARCHIVE}]),
            id="duplicate-name",
        ),
    ],
)
def test_an_unusable_release_response_is_refused(payload: object) -> None:
    """Refuse a response that could describe another release or an ambiguity."""
    with pytest.raises(FinalizePolicyError) as error:
        parse_release_state(payload, tag="v1.2.3")

    assert str(error.value), "the refusal must say what made the response unusable"


@pytest.mark.parametrize(
    ("is_draft", "is_prerelease", "wanted", "expected"),
    [
        pytest.param(False, False, False, True, id="public-final"),
        pytest.param(False, True, True, True, id="public-prerelease"),
        pytest.param(False, True, False, False, id="public-wrong-flag"),
        pytest.param(True, False, False, False, id="still-a-draft"),
    ],
)
def test_already_finalized_requires_a_public_release_with_the_wanted_flag(
    *, is_draft: bool, is_prerelease: bool, wanted: bool, expected: bool
) -> None:
    """Treat only a public release with an agreeing flag as finished."""
    state = ReleaseState("v1.2.3", is_draft, is_prerelease, {})

    assert is_already_finalized(state, is_prerelease=wanted) is expected, (
        f"draft={is_draft} prerelease={is_prerelease} wanted={wanted} must be "
        f"{expected}"
    )


@pytest.mark.parametrize(
    ("is_draft", "is_prerelease", "wanted", "expected"),
    [
        pytest.param(False, False, False, Readback.FINALIZED, id="public-final"),
        pytest.param(False, True, False, Readback.PRERELEASE_MISMATCH, id="wrong-flag"),
        pytest.param(True, False, False, Readback.STILL_DRAFT, id="still-a-draft"),
        pytest.param(True, True, False, Readback.STILL_DRAFT, id="draft-wins"),
    ],
)
def test_readback_separates_a_draft_from_a_wrong_flag(
    *, is_draft: bool, is_prerelease: bool, wanted: bool, expected: Readback
) -> None:
    """Give each failure its own diagnosis, since neither is retryable alike."""
    state = ReleaseState("v1.2.3", is_draft, is_prerelease, {})

    assert classify_readback(state, is_prerelease=wanted) is expected, (
        f"draft={is_draft} prerelease={is_prerelease} wanted={wanted} must be "
        f"{expected}"
    )
