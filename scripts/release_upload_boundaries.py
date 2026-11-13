"""Name which upload boundary a ``gh`` invocation crosses.

A run reaches GitHub four ways, and the four are worth separate numbers: it
lists the release to decide what is missing, it downloads the assets the
listing hid no digest for, it sends the transfer, and it lists the release
again to describe what arrived. The last two are the same command with a
different meaning, which is why naming them is a decision rather than a lookup.

This module owns only that naming. It launches nothing, measures nothing, and
keeps no state: :func:`boundary_of` is a total function of the command and the
phase the run has reached, so a test states a phase and asserts a name without
a subprocess or a clock. Timing is :mod:`release_upload_timing`'s, and it asks
this module which boundary a call was.

A listing's name depends on *when* it happens, and the phase is an argument
rather than a count of listings already seen. Counting makes the meaning of a
call depend on history -- a run that lists once more for a reason added later
would silently renumber the figures an operator reads -- and it hides the
dependency from the caller who knows which moment it is in.

Run via :class:`release_upload_timing.TimedRunner`; behavioural coverage lives
in ``scripts/tests/test_release_upload_boundaries.py``.
"""

import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc

__all__ = [
    "PHASE_CONFIRMATION",
    "PHASE_RECONCILIATION",
    "UPLOAD_BOUNDARY",
    "boundary_of",
    "is_release_command",
]

#: Where the ``gh`` subcommand sits in an argument list: ``gh``, ``release``,
#: then the subcommand that names what is being done with the release.
_SUBCOMMAND_INDEX = 2

#: The subcommand that reads the release, as opposed to transferring to it.
_LIST_SUBCOMMAND = "view"

#: The subcommands that cross a boundary of their own, and which one each is.
#: ``view`` is absent because which listing a call is depends on *when* it
#: happens, not on what it runs.
_SUBCOMMAND_BOUNDARIES = {"download": "download", "upload": "upload"}

#: The boundary whose crossing ends the reconciliation phase. Sending a
#: transfer is the event that turns "what is the release missing" into "what
#: does the release hold", so the phase change is a consequence of a named
#: call rather than of a caller remembering to announce it.
UPLOAD_BOUNDARY = "upload"

#: The listing taken to decide what the release is missing, before any
#: transfer.
PHASE_RECONCILIATION = "reconciliation"

#: The listing taken after a transfer to describe what GitHub now holds.
PHASE_CONFIRMATION = "confirmation"

#: Which boundary a listing names, by the phase it is taken in. This is the
#: whole of the "which listing is this" question.
_LISTING_BOUNDARY_FOR_PHASE = {
    PHASE_RECONCILIATION: "listing",
    PHASE_CONFIRMATION: "verification",
}


def boundary_of(arguments: cabc.Sequence[str], *, phase: str) -> str | None:
    """Name the boundary a command is, or ``None`` when it is not one.

    Parameters
    ----------
    arguments
        The command's argument list, as a runner receives it.
    phase
        Which moment the run has reached: :data:`PHASE_RECONCILIATION` before
        any transfer, :data:`PHASE_CONFIRMATION` after one.

    Returns
    -------
    str | None
        The boundary name, or ``None`` when the command is not a boundary.

    Notes
    -----
    ``download`` and ``upload`` name themselves and ignore the phase: they
    only ever mean one thing. A command that is not a ``gh release``
    invocation at all is not a boundary and is passed through unmeasured --
    attributing its duration to one of the four would be inventing a fact.

    A ``phase`` that is neither of the two names raises ``KeyError`` rather
    than yielding a third boundary: no listing boundary exists for a moment
    this module has never been told about, and guessing one would mislabel the
    figure. It is named here rather than in a ``Raises`` section, which Ruff's
    DOC501 reserves for exceptions a function raises itself, and it comes from
    the lookup table below.

    Examples
    --------
    >>> boundary_of(["gh", "release", "view", "v1"], phase=PHASE_RECONCILIATION)
    'listing'
    >>> boundary_of(["gh", "release", "view", "v1"], phase=PHASE_CONFIRMATION)
    'verification'
    >>> boundary_of(
    ...     ["gh", "release", "upload", "v1", "a.deb"], phase=PHASE_RECONCILIATION
    ... )
    'upload'
    >>> boundary_of(["gh", "pr", "view"], phase=PHASE_RECONCILIATION) is None
    True
    """
    if not is_release_command(arguments):
        return None
    subcommand = arguments[_SUBCOMMAND_INDEX]
    if subcommand == _LIST_SUBCOMMAND:
        return _LISTING_BOUNDARY_FOR_PHASE[phase]
    return _SUBCOMMAND_BOUNDARIES.get(subcommand)


def is_release_command(arguments: cabc.Sequence[str]) -> bool:
    """Report whether these arguments are a complete ``gh release`` command.

    Returns
    -------
    bool
        ``True`` when the executable is ``gh``, the command is ``release``,
        and a subcommand follows.

    Examples
    --------
    >>> is_release_command(["gh", "release", "view", "v1"])
    True
    >>> is_release_command(["gh", "release"])
    False
    >>> is_release_command(["python3", "release", "view"])
    False
    """
    return list(arguments[:2]) == ["gh", "release"] and (
        len(arguments) > _SUBCOMMAND_INDEX
    )
