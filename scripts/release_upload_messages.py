"""Render the refusal an upload run reports when it declines to transfer.

The two kinds of blockage a run can find need different words because they
ask different things of an operator. A conflict means the release holds bytes
that were read and disagreed with the trusted record, so somebody has to
decide which account is right. Unverifiable means nothing was compared at all,
so a retry may be the whole remedy. A run that said "disagree" for both would
promise evidence it does not have and send an operator hunting for wrong
bytes that may be correct.

The prose lives apart from :mod:`release_upload_plan` because it is read by a
person rather than by a job: the plan's fields are a contract with the
pipeline, while these sentences are a contract with whoever is on call. The
split also keeps a run with both kinds from reporting one and dropping the
other, which is why :func:`blocked_message` composes rather than leaving it to
a caller.

Run via ``scripts/upload_declared_assets_cli.py``; behavioural coverage lives
in ``scripts/tests/test_release_upload_plan.py``.
"""

import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc

    from release_upload_failures import DigestFailure

__all__ = ["blocked_message", "conflict_message", "unverifiable_message"]


def conflict_message(conflicts: cabc.Iterable[str]) -> str:
    """Describe the attached assets a run refused to overwrite.

    Parameters
    ----------
    conflicts
        The names whose attached bytes disagree with the trusted record.

    Returns
    -------
    str
        One line naming every conflicting asset.

    Examples
    --------
    >>> conflict_message(["a.deb", "b.rpm"])
    'the release holds assets that disagree with the trusted record: a.deb, b.rpm'
    """
    return (
        "the release holds assets that disagree with the trusted record: "
        + ", ".join(conflicts)
    )


def unverifiable_message(
    unverifiable: cabc.Iterable[str],
    reasons: cabc.Mapping[str, DigestFailure] | None = None,
) -> str:
    """Describe the attached assets whose bytes a run could not read.

    Parameters
    ----------
    unverifiable
        The names attached to the release whose digest neither the listing nor
        a download could establish.
    reasons
        Name mapped to the bounded category explaining it. Each name's category
        and suggested remedy are appended to the clause; a name with no known
        category is still named, because the refusal stands without it.

    Returns
    -------
    str
        One line naming every unreadable asset, with each known reason.

    Notes
    -----
    The wording deliberately does not say the bytes disagree. Nothing compared
    them: GitHub reported no digest and the download did not yield the file.
    Reporting that as a disagreement would promise evidence the run does not
    have and send an operator looking for wrong bytes that may be correct.

    The reasons are rendered beside their names rather than replacing them, so
    the sentence still names every asset the run refused over, and the remedy
    is what tells an operator whether a retry is worth attempting.

    Examples
    --------
    >>> unverifiable_message(["a.deb"]) == (
    ...     "the release attaches assets whose bytes could not be verified: a.deb"
    ... )
    True
    >>> from release_upload_failures import DigestFailure
    >>> unverifiable_message(
    ...     ["a.deb"], {"a.deb": DigestFailure.DOWNLOAD_FAILED}
    ... ) == (
    ...     "the release attaches assets whose bytes could not be verified: "
    ...     "a.deb (download-failed: retry the download)"
    ... )
    True
    """
    known = reasons or {}
    described = ", ".join(
        f"{name} ({known[name].value}: {known[name].remedy})" if name in known else name
        for name in unverifiable
    )
    return "the release attaches assets whose bytes could not be verified: " + described


def blocked_message(
    conflicts: cabc.Iterable[str],
    unverifiable: cabc.Iterable[str],
    reasons: cabc.Mapping[str, DigestFailure] | None = None,
) -> str:
    """Describe every reason the run refused, one clause per kind present.

    Parameters
    ----------
    conflicts
        The names whose attached bytes were read and disagreed.
    unverifiable
        The names whose attached bytes could not be read.
    reasons
        Unverifiable name mapped to the bounded category explaining it, as
        :func:`unverifiable_message` renders them.

    Returns
    -------
    str
        One line, naming each distinct reason that actually applies.

    Notes
    -----
    Kept whole rather than composed by the caller so a run with both kinds
    cannot report one and drop the other. Only the clauses with names are
    emitted: reporting "no unverifiable assets" beside a conflict would bury
    the finding in a statement about an absence.

    Examples
    --------
    >>> blocked_message(["a.deb"], ["b.rpm"]) == (
    ...     "the release holds assets that disagree with the trusted record: "
    ...     "a.deb; the release attaches assets whose bytes could not be "
    ...     "verified: b.rpm"
    ... )
    True
    >>> blocked_message(["a.deb"], [])
    'the release holds assets that disagree with the trusted record: a.deb'
    >>> blocked_message([], [])
    ''
    """
    disputed = tuple(conflicts)
    unread = tuple(unverifiable)
    clauses = [
        conflict_message(disputed) if disputed else "",
        unverifiable_message(unread, reasons) if unread else "",
    ]
    return "; ".join(clause for clause in clauses if clause)
