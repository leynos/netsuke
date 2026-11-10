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

The refusal an operator reads is prose rather than a record, so it lives in
:mod:`release_upload_messages`; this module keeps only what a job parses.

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
    "PlanRecord",
    "UploadRequest",
    "read_plan",
    "write_plan",
]

#: Name of the JSON plan the step exposes to the job.
PLAN_NAME = "upload-plan.json"


class PlanRecord(typ.TypedDict):
    """Describe the plan as it is written, for a reader of the file.

    Attributes
    ----------
    actions
        One ``[name, action]`` pair per decided asset, in the order decided.
    conflicts
        The names whose attached bytes were read and disagreed.
    unverifiable
        The names whose attached bytes could not be read at all.
    unverifiable_reasons
        Unverifiable name mapped to the bounded :class:`release_upload_failures.
        DigestFailure` spelling that explains it.

    Notes
    -----
    The writer's own type, declared beside it, so a reader that decodes the
    file names the same shape the writer serialized. Without one, a test reads
    a ``dict[str, object]`` and has to narrow every field by hand; with two
    copies, the reader and the writer can drift while each stays self-consistent.

    The fields are the wide types JSON actually yields. ``actions`` pairs are
    lists rather than tuples because that is what the file holds, and the
    point of the record is to describe the encoding rather than a richer value
    a caller might prefer.
    """

    actions: list[list[str]]
    conflicts: list[str]
    unverifiable: list[str]
    unverifiable_reasons: dict[str, str]


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


def read_plan(plan_path: pathlib.Path) -> PlanRecord:
    """Return the plan at ``plan_path``, decoded to the shape written.

    Parameters
    ----------
    plan_path
        The JSON plan :func:`write_plan` wrote.

    Returns
    -------
    PlanRecord
        The decoded plan.

    Notes
    -----
    Decoding here rather than at each reader keeps the reader and the writer on
    one declaration: a case that reads the file names the same fields the
    writer serialized, and a field renamed on one side fails to typecheck on
    the other instead of silently reading as an unrelated JSON value. The cast
    is the JSON boundary itself -- nothing validates the file's contents, and
    a plan a run produced is read by cases that go on to assert its shape.

    Examples
    --------
    >>> import pathlib
    >>> import tempfile
    >>> from release_reconcile_github import AssetAction
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     destination = pathlib.Path(tmp) / "plan.json"
    ...     write_plan(destination, [("a", AssetAction.SKIP_VERIFIED)], Blockers())
    ...     read_plan(destination)["actions"]
    [['a', 'skip-verified']]
    """
    decoded: PlanRecord = json.loads(plan_path.read_text(encoding="utf-8"))
    return decoded
