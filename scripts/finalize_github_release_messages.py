"""Render the diagnosis an operator reads when a release is not finalized.

A decision names what happened; these sentences are what a person on call sees
in the workflow log. Keeping them out of the command module lets the words
change without changing what the step decides, and lets the same outcome render
the same way however it was reached.

Every clause is derived from the outcome alone, with the invocation supplying
only the facts a refusal cannot carry: the plan's path and the candidate's tag
and prerelease flag. Nothing here reads a file, starts a process, or names an
exit status.

One function renders each verdict, and :data:`_RENDERERS` is the only place
that decides which one. The table is exhaustive over the verdicts this run can
report, so a verdict added to the decision without a sentence is caught by the
command line's own coverage rather than by an operator meeting raw data.

>>> from finalize_github_release_decisions import PublishOutcome, PublishVerdict
>>> describe(
...     PublishOutcome(
...         PublishVerdict.UPLOAD_REQUIRED, source="plan", uploads=("a.deb",)
...     ),
...     plan_path="plan.json",
...     tag="v1.2.3",
...     is_prerelease=False,
... ) == (
...     "plan still requires uploads (a.deb); run the declared upload "
...     "contract before finalizing; this run has not edited the release"
... )
True

Run via ``scripts/finalize_github_release.py``; behavioural coverage lives in
``scripts/tests/test_finalize_github_release_publish.py``.
"""

import dataclasses

# ``Diagnosis`` reads its field annotations from the class at runtime.
import pathlib  # ruff: ignore[typing-only-standard-library-import]
import typing as typ

import finalize_github_release_decisions as decisions
import finalize_github_release_remote as release_remote
import release_upload_messages

if typ.TYPE_CHECKING:
    import collections.abc as cabc

__all__ = ["describe"]


@dataclasses.dataclass(frozen=True, slots=True)
class Diagnosis:
    """Name the invocation facts a refusal cannot carry itself.

    Attributes
    ----------
    plan_path
        The plan the invocation named, so an input fault can name it.
    tag
        The tag the invocation is finalizing. It reaches here only after the
        candidate record has been checked against it, so it names the same
        release the candidate does.
    is_prerelease
        The flag the validated candidate requires, never the live release's.
    """

    plan_path: pathlib.Path | str
    tag: str
    is_prerelease: bool


def draft_state(outcome: decisions.PublishOutcome) -> str:
    """Name the release state only as far as this run has established it.

    Parameters
    ----------
    outcome
        The refusal being described.

    Returns
    -------
    str
        The clause a refusal appends. A refusal made from the plan alone has
        read nothing, and a retry can begin after an earlier attempt already
        published the release, so it may not claim the release is a draft.

    Examples
    --------
    >>> draft_state(decisions.PublishOutcome(decisions.PublishVerdict.REFUSED))
    'this run has not edited the release'
    >>> draft_state(
    ...     decisions.PublishOutcome(decisions.PublishVerdict.REFUSED, is_draft=True)
    ... )
    'the release remains a draft'
    """
    if outcome.is_draft:
        return "the release remains a draft"
    return "this run has not edited the release"


def blocked(outcome: decisions.PublishOutcome) -> str:
    """Describe the assets that stopped the run, or its own explanation.

    Parameters
    ----------
    outcome
        The refusal being described.

    Returns
    -------
    str
        One clause naming every reason present, or the source's own detail
        when no asset is named.

    Examples
    --------
    >>> blocked(
    ...     decisions.PublishOutcome(
    ...         decisions.PublishVerdict.REFUSED, conflicts=("a.deb",)
    ...     )
    ... )
    'the release holds assets that disagree with the trusted record: a.deb'
    """
    return (
        release_upload_messages.blocked_message(outcome.conflicts, outcome.unverifiable)
        or outcome.detail
    )


def _unreadable(outcome: decisions.PublishOutcome, context: Diagnosis) -> str:
    """Name the plan that could not be decoded."""
    return f"{context.plan_path} is not a usable plan: {outcome.detail}"


def _upload_required(outcome: decisions.PublishOutcome, _context: Diagnosis) -> str:
    """Name the uploads the plan still owes, and that none was attempted."""
    return (
        f"{outcome.source} still requires uploads "
        f"({', '.join(outcome.uploads)}); run the declared upload "
        f"contract before finalizing; {draft_state(outcome)}"
    )


def _refused(outcome: decisions.PublishOutcome, _context: Diagnosis) -> str:
    """Name the assets that stopped the run, and the state they leave."""
    return f"{blocked(outcome)}; {draft_state(outcome)}"


def _not_a_draft(outcome: decisions.PublishOutcome, _context: Diagnosis) -> str:
    """Refuse a public release the candidate would have to re-classify."""
    return (
        "the release is public but its prerelease state contradicts "
        "the validated candidate; a public release cannot be made a "
        "draft again"
    )


def _prerelease_mismatch(outcome: decisions.PublishOutcome, _context: Diagnosis) -> str:
    """Report the read-back that published the release with the wrong flag."""
    return (
        "the release is public but its prerelease state contradicts "
        "the validated candidate; it cannot be made a draft again"
    )


def _published_mismatch(outcome: decisions.PublishOutcome, context: Diagnosis) -> str:
    """Name a public release whose inventory this run cannot repair."""
    mismatch = blocked(outcome) or (
        f"{len(outcome.uploads)} declared asset(s) are no longer attached"
    )
    return (
        f"release {context.tag} is public, but {mismatch}; GitHub "
        "cannot make a public release a draft again, so this run "
        "cannot repair it: replace the named asset, or withdraw the "
        "release, before announcing it"
    )


def _remote_failed(outcome: decisions.PublishOutcome, _context: Diagnosis) -> str:
    """Append what a failed call leaves established about the release."""
    state = release_remote.unknown_state_message(after_edit=outcome.after_edit)
    return f"{outcome.detail}. {state}"


def _still_draft(outcome: decisions.PublishOutcome, _context: Diagnosis) -> str:
    """Report an accepted edit that left the release a draft."""
    return (
        "gh accepted the edit but the release is still a draft; no "
        "asset was replaced and the release was not published"
    )


def _public_release(outcome: decisions.PublishOutcome, context: Diagnosis) -> str:
    """Render the last resort: a release this run did make public.

    Returns
    -------
    str
        The sentence recording what the run achieved, kept as a fallback so an
        unexpected verdict still reads as a success rather than as nothing.
    """
    return (
        f"release {context.tag} is public (outcome {outcome.verdict.value}, "
        f"prerelease {context.is_prerelease})"
    )


type _Renderer = cabc.Callable[[decisions.PublishOutcome, Diagnosis], str]

#: The sentence each verdict renders. A verdict missing from this table is one
#: this run made public, which :func:`_public_release` describes.
_RENDERERS: dict[decisions.PublishVerdict, _Renderer] = {
    decisions.PublishVerdict.INPUT_UNREADABLE: _unreadable,
    decisions.PublishVerdict.UPLOAD_REQUIRED: _upload_required,
    decisions.PublishVerdict.REFUSED: _refused,
    decisions.PublishVerdict.NOT_A_DRAFT: _not_a_draft,
    decisions.PublishVerdict.PRERELEASE_MISMATCH: _prerelease_mismatch,
    decisions.PublishVerdict.PUBLISHED_MISMATCH: _published_mismatch,
    decisions.PublishVerdict.REMOTE_FAILED: _remote_failed,
    decisions.PublishVerdict.STILL_DRAFT: _still_draft,
}


def describe(
    outcome: decisions.PublishOutcome,
    *,
    plan_path: pathlib.Path | str,
    tag: str,
    is_prerelease: bool,
) -> str:
    """Render the diagnosis for a verdict that did not publish the release.

    Parameters
    ----------
    outcome
        The verdict and the facts it rests on.
    plan_path
        The plan the invocation named, so an input fault can name it.
    tag
        The tag the invocation is finalizing. It reaches here only after the
        candidate record has been checked against it, so it names the same
        release the candidate does.
    is_prerelease
        The flag the validated candidate requires, never the live release's.

    Returns
    -------
    str
        The sentence an operator reads. Every fact it states comes from the
        outcome or the invocation, so the same pair always reads the same way.

    Examples
    --------
    >>> from finalize_github_release_state import ReleaseState
    >>> outcome = decisions.draft_gate(ReleaseState("v1.2.3", False, False, {}))
    >>> "cannot be made a draft again" in describe(
    ...     outcome, plan_path="plan.json", tag="v1.2.3", is_prerelease=False
    ... )
    True
    """
    context = Diagnosis(plan_path=plan_path, tag=tag, is_prerelease=is_prerelease)
    render = _RENDERERS.get(outcome.verdict, _public_release)
    return render(outcome, context)
