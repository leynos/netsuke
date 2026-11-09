"""Decide what a recorded release-reconciliation plan permits.

The planner records one action per declared asset. This module turns that
document into the single decision the finalizer needs: whether every asset is
already correct, whether an upload is still owed, or whether the plan refuses
publication. It also carries the decoding fault a malformed plan produces, so
an unreadable plan is a verdict rather than an exception the adapter has to
anticipate at every call site.

Nothing here reads a file or runs a command; the adapter decodes the JSON and
asks :func:`assess_plan` what it means.

Run via ``scripts/finalize_github_release.py``; behavioural coverage lives in
``scripts/tests/test_finalize_github_release_state.py``.
"""

import collections.abc as cabc
import dataclasses
import enum

import release_reconcile_github


class FinalizePolicyError(ValueError):
    """Report a recorded value that cannot support a finalization decision."""


class PlanVerdict(enum.Enum):
    """What a recorded reconciliation plan says about publishing."""

    #: Every declared asset is present with the expected digest.
    VERIFIED = "verified"

    #: At least one declared asset is absent and must be uploaded first.
    UPLOAD_REQUIRED = "upload-required"

    #: An asset disagrees with the trusted record, or could not be read.
    REFUSED = "refused"

    #: The plan document itself is not shaped like a plan.
    UNREADABLE = "unreadable"


_ACTION_VALUES = frozenset(
    action.value for action in release_reconcile_github.AssetAction
)
_UPLOAD = release_reconcile_github.AssetAction.UPLOAD.value
_CONFLICT = release_reconcile_github.AssetAction.CONFLICT.value
_UNVERIFIABLE = release_reconcile_github.AssetAction.UNVERIFIABLE.value

#: A plan row is exactly an asset name followed by its action.
_ROW_WIDTH = 2


@dataclasses.dataclass(frozen=True, slots=True)
class PlanAssessment:
    """Record what a plan permits and which assets stopped it.

    Attributes
    ----------
    verdict
        Whether the plan permits publication, requires an upload, refuses, or
        could not be decoded.
    conflicts
        Names whose recorded bytes disagree with the trusted record.
    unverifiable
        Names whose recorded digest could not be established.
    uploads
        Names the plan still expects to upload.
    detail
        Why the document could not be read, for an
        :attr:`PlanVerdict.UNREADABLE` verdict; empty otherwise.
    """

    verdict: PlanVerdict
    conflicts: tuple[str, ...] = ()
    unverifiable: tuple[str, ...] = ()
    uploads: tuple[str, ...] = ()
    detail: str = ""


def _plan_rows(plan: cabc.Mapping[str, object]) -> tuple[tuple[str, str], ...]:
    """Decode the plan's per-asset rows, refusing any other shape.

    Returns
    -------
    tuple[tuple[str, str], ...]
        One ``(name, action)`` pair per row, in the recorded order.

    Raises
    ------
    FinalizePolicyError
        If the rows are not pairs of a non-empty name and a known action.

    Examples
    --------
    >>> _plan_rows({"actions": [["a", "skip-verified"]]})
    (('a', 'skip-verified'),)
    """
    raw_rows = plan.get("actions")
    if not isinstance(raw_rows, list):
        msg = "the plan does not carry an actions list"
        raise FinalizePolicyError(msg)
    rows: list[tuple[str, str]] = []
    for row in raw_rows:
        if not isinstance(row, (list, tuple)) or len(row) != _ROW_WIDTH:
            msg = f"the plan row {row!r} is not a name and action pair"
            raise FinalizePolicyError(msg)
        name, action = row
        if not isinstance(name, str) or not name:
            msg = f"the plan row {row!r} does not name an asset"
            raise FinalizePolicyError(msg)
        if not isinstance(action, str) or action not in _ACTION_VALUES:
            msg = f"the plan row {row!r} carries an unknown action"
            raise FinalizePolicyError(msg)
        rows.append((name, action))
    return tuple(rows)


def _named(plan: cabc.Mapping[str, object], member: str) -> tuple[str, ...]:
    """Decode one optional list of asset names from the plan.

    Returns
    -------
    tuple[str, ...]
        The recorded names, or an empty tuple when the member is absent.

    Raises
    ------
    FinalizePolicyError
        If the member is not a list of non-empty strings.

    Examples
    --------
    >>> _named({}, "conflicts")
    ()
    >>> _named({"conflicts": ["a"]}, "conflicts")
    ('a',)
    """
    raw = plan.get(member, [])
    if not isinstance(raw, list) or not all(
        isinstance(name, str) and name for name in raw
    ):
        msg = f"the plan's {member} member is not a list of asset names"
        raise FinalizePolicyError(msg)
    return tuple(raw)


def _selected(rows: tuple[tuple[str, str], ...], action: str) -> tuple[str, ...]:
    """Return the names one action applies to, preserving the recorded order."""
    return tuple(name for name, recorded in rows if recorded == action)


def assess_plan(plan: object) -> PlanAssessment:
    """Decide whether a recorded reconciliation plan permits publication.

    Parameters
    ----------
    plan
        The decoded JSON document ``finalize_github_release_cli`` wrote.

    Returns
    -------
    PlanAssessment
        The verdict, the names that justify it, and any decoding fault.

    Notes
    -----
    A conflict outranks an upload, matching the planner's own precedence. The
    plan's top-level name lists are unioned with the per-asset rows because the
    two are written from the same decision but neither is derivable from the
    other: a hand-edited plan may carry a row its summary omits, and a plan
    from an older producer may carry only the summary.

    An empty action list is refused rather than treated as fully verified. No
    actions also describes a release with nothing declared, and un-drafting on
    no evidence is the one outcome this stage exists to prevent.

    Examples
    --------
    >>> assess_plan({"actions": [["a", "skip-verified"]]}).verdict.value
    'verified'
    >>> verdict = assess_plan({"actions": [["a", "upload"]]}).verdict
    >>> verdict.value
    'upload-required'
    >>> assessment = assess_plan(
    ...     {"actions": [["a", "conflict"]], "conflicts": ["a"]}
    ... )
    >>> assessment.verdict.value, assessment.conflicts
    ('refused', ('a',))
    >>> assess_plan({"actions": []}).verdict.value
    'refused'
    >>> assess_plan({"assets": {}}).verdict.value
    'unreadable'
    """
    if not isinstance(plan, cabc.Mapping):
        return PlanAssessment(
            PlanVerdict.UNREADABLE, detail="the plan is not a JSON object"
        )
    try:
        rows = _plan_rows(plan)
        conflicts = (*_named(plan, "conflicts"), *_selected(rows, _CONFLICT))
        unverifiable = (
            *_named(plan, "unverifiable"),
            *_selected(rows, _UNVERIFIABLE),
        )
    except FinalizePolicyError as error:
        return PlanAssessment(PlanVerdict.UNREADABLE, detail=str(error))
    if not rows:
        return PlanAssessment(
            PlanVerdict.REFUSED,
            detail=(
                "the plan carries no asset decisions, so it is "
                "indistinguishable from a release nothing was checked against"
            ),
        )
    if conflicts or unverifiable:
        return PlanAssessment(
            PlanVerdict.REFUSED,
            conflicts=_unique(conflicts),
            unverifiable=_unique(unverifiable),
        )
    uploads = _selected(rows, _UPLOAD)
    if uploads:
        return PlanAssessment(PlanVerdict.UPLOAD_REQUIRED, uploads=_unique(uploads))
    return PlanAssessment(PlanVerdict.VERIFIED)


def _unique(names: cabc.Iterable[str]) -> tuple[str, ...]:
    """Return names in first-seen order without repetition.

    Returns
    -------
    tuple[str, ...]
        The distinct names, in the order they first appeared.

    Examples
    --------
    >>> _unique(["b", "a", "b"])
    ('b', 'a')
    """
    return tuple(dict.fromkeys(names))
