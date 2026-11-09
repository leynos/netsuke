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

import release_reconcile_github

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import pathlib

    import release_upload_failures

__all__ = [
    "PLAN_NAME",
    "Blockers",
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


@dataclasses.dataclass(frozen=True, slots=True)
class Blockers:
    """Hold what stopped an upload run, split into the kinds a reader needs.

    Attributes
    ----------
    conflicts
        The names whose attached bytes were read and disagreed with the
        trusted record.
    unverifiable
        The names whose attached bytes could not be read at all. Recorded
        separately from ``conflicts`` because the operator's next step
        differs: one is a wrong release, the other may be a broken listing.
    reasons
        Unverifiable asset name mapped to the bounded category that explains
        it, such as ``download-failed``. Names with no known category are
        simply absent, which is why the mapping cannot be read as a
        replacement for ``unverifiable``.

    Notes
    -----
    One value rather than three parameters, because the three are one finding:
    whether a run is blocked, and by what. A caller able to pass the conflicts
    from one observation and the unverifiable names from another could write a
    plan that disagrees with itself, and no reader could tell which half was
    current. Empty defaults describe an unblocked run, which is the common case
    and the one worth spelling as nothing at all.

    Examples
    --------
    >>> Blockers()
    Blockers(conflicts=(), unverifiable=(), reasons={})
    >>> Blockers(conflicts=("a.deb",)).conflicts
    ('a.deb',)
    """

    conflicts: tuple[str, ...] = ()
    unverifiable: tuple[str, ...] = ()
    reasons: cabc.Mapping[str, release_upload_failures.DigestFailure] = (
        dataclasses.field(default_factory=dict)
    )

    @classmethod
    def of(
        cls,
        actions: cabc.Iterable[tuple[str, release_reconcile_github.AssetAction]],
        reasons: cabc.Mapping[str, release_upload_failures.DigestFailure] | None = None,
    ) -> Blockers:
        """Derive every blocker from a plan and the categories behind it.

        Parameters
        ----------
        actions
            One ``(name, action)`` pair per declared asset, as
            :func:`release_upload_observation.plan_release` returned them.
        reasons
            Unverifiable asset name mapped to the bounded category explaining
            it.

        Returns
        -------
        Blockers
            The conflicting and unverifiable names the plan holds, with the
            categories that were supplied for them.

        Notes
        -----
        Derived from ``actions`` rather than passed in alongside them so a
        caller cannot report a name as blocking in one place and as settled in
        the other. Only the unverifiable names keep their reasons: a reason
        supplied for an asset that verified describes a digest that exists.

        Examples
        --------
        >>> from release_reconcile_github import AssetAction
        >>> Blockers.of([("a", AssetAction.CONFLICT), ("b", AssetAction.UPLOAD)])
        Blockers(conflicts=('a',), unverifiable=(), reasons={})
        """
        settled = tuple(actions)
        unread = release_reconcile_github.unverifiable(settled)
        known = reasons or {}
        return cls(
            conflicts=release_reconcile_github.conflicts(settled),
            unverifiable=unread,
            reasons={name: known[name] for name in unread if name in known},
        )


def write_plan(
    plan_path: pathlib.Path,
    actions: cabc.Iterable[tuple[str, release_reconcile_github.AssetAction]],
    blockers: Blockers,
) -> None:
    """Write the per-asset plan in the shape the release jobs already read.

    Parameters
    ----------
    plan_path
        Destination for the JSON plan.
    actions
        One ``(name, action)`` pair per declared asset.
    blockers
        What stopped the run, split into the two kinds a reader must tell
        apart, with the bounded category behind each unverifiable name.

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

    The categories are recorded rather than the underlying error text. A plan
    is retained beside the release for a job to read, so anything written there
    is published: an error message from a failing command can carry a URL, a
    temporary path, or a token a tool chose to echo, and none of that belongs
    in an artefact. A bounded vocabulary is also what lets a reader branch on
    the reason at all.

    Examples
    --------
    >>> import pathlib
    >>> import tempfile
    >>> from release_reconcile_github import AssetAction
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     destination = pathlib.Path(tmp) / "plan.json"
    ...     write_plan(destination, [("a", AssetAction.UPLOAD)], Blockers())
    ...     print(destination.read_text(encoding="utf-8"))
    {
      "actions": [
        [
          "a",
          "upload"
        ]
      ],
      "conflicts": [],
      "unverifiable": [],
      "unverifiable_reasons": {}
    }
    <BLANKLINE>
    """
    plan_path.write_text(
        json.dumps(
            {
                "actions": [[name, action.value] for name, action in actions],
                "conflicts": list(blockers.conflicts),
                "unverifiable": list(blockers.unverifiable),
                "unverifiable_reasons": {
                    name: failure.value for name, failure in blockers.reasons.items()
                },
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


def unverifiable_message(
    unverifiable: cabc.Iterable[str],
    reasons: cabc.Mapping[str, release_upload_failures.DigestFailure] | None = None,
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
    >>> unverifiable_message(
    ...     ["a.deb"], {"a.deb": release_upload_failures.DigestFailure.DOWNLOAD_FAILED}
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
    reasons: cabc.Mapping[str, release_upload_failures.DigestFailure] | None = None,
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
