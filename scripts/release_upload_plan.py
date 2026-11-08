"""Describe one upload run and record the plan it produced.

The uploader's two data-shaped concerns live here: what a run is asked to do
(:class:`UploadRequest`) and what it decided (:func:`write_plan`). They are
apart from the decision itself so a caller that only wants to inspect or
re-serialize a plan need not import the network boundary.

The plan's shape matches ``finalize_github_release_cli``'s on purpose: a job
that reads one release plan can read the other, and an operator comparing the
two is comparing decisions rather than formats.

The plan is written for a refusing run too. An operator who sees only ``the
release holds assets that disagree`` still has to guess which assets were
planned for upload; the record answers that without a second attempt.

Run via ``.github/actions/upload-declared-assets``; behavioural coverage lives
in ``scripts/tests/test_upload_declared_assets_cli.py``.
"""

import dataclasses
import json
import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import pathlib

    import release_reconcile_github

__all__ = [
    "PLAN_NAME",
    "UploadRequest",
    "blocked_message",
    "conflict_message",
    "write_plan",
]

#: Name of the JSON plan the step exposes to the job.
PLAN_NAME = "upload-plan.json"


@dataclasses.dataclass(frozen=True, slots=True)
class UploadRequest:
    """Name everything one upload run needs, in one value.

    Attributes
    ----------
    tag
        The validated release tag to reconcile.
    payload_dir
        Directory holding exactly the selected assets, under published names.
    digest_record
        The declaration record: digests the declaration stage wrote from the
        staged tree, which is what the payload and the release are judged
        against.
    selection_record
        The selection record: digests the selection stage wrote from the
        staged sources it verified, before copying them. Upload compares the
        two records so neither is the only account of the release.
    plan_path
        Where to write the per-asset reconciliation plan.
    is_dry_run
        When true, plan against the release but upload nothing.

    Examples
    --------
    >>> import pathlib
    >>> request = UploadRequest(
    ...     tag="v1.0.0", payload_dir=pathlib.Path("payload"),
    ...     digest_record=pathlib.Path("trusted.json"),
    ...     selection_record=pathlib.Path("selection.json"),
    ...     plan_path=pathlib.Path("plan.json"), is_dry_run=True,
    ... )
    >>> request.tag, request.is_dry_run
    ('v1.0.0', True)
    """

    tag: str
    payload_dir: pathlib.Path
    digest_record: pathlib.Path
    selection_record: pathlib.Path
    plan_path: pathlib.Path
    is_dry_run: bool


def write_plan(
    plan_path: pathlib.Path,
    actions: cabc.Iterable[tuple[str, release_reconcile_github.AssetAction]],
    conflicts: cabc.Iterable[str],
    unverifiable: cabc.Iterable[str] = (),
) -> None:
    """Write the per-asset plan in the shape the release jobs already read.

    Parameters
    ----------
    plan_path
        Destination for the JSON plan.
    actions
        One ``(name, action)`` pair per declared asset.
    conflicts
        The names whose attached bytes were read and disagreed with the
        trusted record.
    unverifiable
        The names whose attached bytes could not be read at all. Recorded
        separately from ``conflicts`` because the operator's next step differs:
        one is a wrong release, the other may be a broken listing.

    Notes
    -----
    JSON is sorted and newline-terminated so identical decisions serialize
    identically, which lets a job diff two runs' plans. An unwritable
    destination propagates ``OSError`` to the caller, which reports it as a
    distinct exit status rather than as an upload failure.

    ``unverifiable`` already appears in ``actions`` under its own action. It is
    repeated at the top level because a reader -- human or job -- that looks
    only at the blocking summary should not have to scan the per-asset rows to
    learn which kind of blockage it is looking at.

    Examples
    --------
    >>> import pathlib
    >>> import tempfile
    >>> from release_reconcile_github import AssetAction
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     destination = pathlib.Path(tmp) / "plan.json"
    ...     write_plan(destination, [("a", AssetAction.UPLOAD)], [])
    ...     print(destination.read_text(encoding="utf-8"))
    {
      "actions": [
        [
          "a",
          "upload"
        ]
      ],
      "conflicts": [],
      "unverifiable": []
    }
    <BLANKLINE>
    """
    plan_path.write_text(
        json.dumps(
            {
                "actions": [[name, action.value] for name, action in actions],
                "conflicts": list(conflicts),
                "unverifiable": list(unverifiable),
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )


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


def unverifiable_message(unverifiable: cabc.Iterable[str]) -> str:
    """Describe the attached assets whose bytes a run could not read.

    Parameters
    ----------
    unverifiable
        The names attached to the release whose digest neither the listing nor
        a download could establish.

    Returns
    -------
    str
        One line naming every unreadable asset.

    Notes
    -----
    The wording deliberately does not say the bytes disagree. Nothing compared
    them: GitHub reported no digest and the download did not yield the file.
    Reporting that as a disagreement would promise evidence the run does not
    have and send an operator looking for wrong bytes that may be correct.

    Examples
    --------
    >>> unverifiable_message(["a.deb"]) == (
    ...     "the release attaches assets whose bytes could not be verified: a.deb"
    ... )
    True
    """
    return (
        "the release attaches assets whose bytes could not be verified: "
        + ", ".join(unverifiable)
    )


def blocked_message(
    conflicts: cabc.Iterable[str], unverifiable: cabc.Iterable[str]
) -> str:
    """Describe every reason the run refused, one clause per kind present.

    Parameters
    ----------
    conflicts
        The names whose attached bytes were read and disagreed.
    unverifiable
        The names whose attached bytes could not be read.

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
        unverifiable_message(unread) if unread else "",
    ]
    return "; ".join(clause for clause in clauses if clause)
