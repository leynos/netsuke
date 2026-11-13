"""Carry what the finalizer's two effects said, and name their ports.

A publishing decision is made from values, and two of those values stand in for
a live release: what a read established, and what an edit replied. Keeping the
two carriers apart from :mod:`finalize_github_release_decisions` lets that
module stay a statement of policy — which release may be un-drafted, and what
the outcome is — while this module states the shapes the policy exchanges with
its adapters. Neither carries prose: the command layer renders the outcome, and
an adapter supplies only the clause naming what went wrong.

Folding a failed read into a value rather than an exception is the point. The
decision can then report "the state is unknown" alongside every other verdict,
and the difference between "not a draft" and "nothing established the draft" is
spelled as a value the decision cannot accidentally lose.

Run via ``scripts/finalize_github_release.py``; behavioural coverage lives in
``scripts/tests/test_finalize_github_release_publish_ports.py``.
"""

import dataclasses
import typing as typ

# ``ReleaseRead`` reads this annotation from the class at runtime.
import finalize_github_release_state as release_state  # ruff: ignore[typing-only-third-party-import]


@dataclasses.dataclass(frozen=True, slots=True)
class ReleaseRead:
    """Carry what one release read established.

    Attributes
    ----------
    state
        The decoded release, or ``None`` when the read established none.
    detail
        The clause naming why no state was decoded. Empty when ``state`` is
        present.

    Notes
    -----
    ``None`` is deliberately not spelled as an empty state: an unread release
    is not a draft, and the difference decides whether the operator is told the
    state is unknown.

    Examples
    --------
    >>> ReleaseRead(detail="gh release view exited 5: HTTP 403").state is None
    True
    """

    state: release_state.ReleaseState | None = None
    detail: str = ""


@dataclasses.dataclass(frozen=True, slots=True)
class RemoteAnswer:
    """Carry what the un-drafting call replied.

    Attributes
    ----------
    exit_code
        The status ``gh`` reported; ``0`` when it accepted the edit.
    detail
        The clause naming the failure, empty on success.

    Examples
    --------
    >>> RemoteAnswer(exit_code=1, detail="gh release edit exited 1: HTTP 502").detail
    'gh release edit exited 1: HTTP 502'
    """

    exit_code: int = 0
    detail: str = ""


class ReleaseReader(typ.Protocol):
    """Read the release as GitHub currently reports it."""

    def __call__(self, *, after_edit: bool) -> ReleaseRead:
        """Return the decoded state, or why none could be decoded."""
        ...


class ReleaseEditor(typ.Protocol):
    """Un-draft the release with the validated candidate's prerelease flag."""

    def __call__(self, *, is_prerelease: bool) -> RemoteAnswer:
        """Return what ``gh`` replied to the edit."""
        ...
