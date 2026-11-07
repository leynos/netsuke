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

__all__ = ["PLAN_NAME", "UploadRequest", "conflict_message", "write_plan"]

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
        The trusted record selection wrote from the staged producer bytes.
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
    ...     plan_path=pathlib.Path("plan.json"), is_dry_run=True,
    ... )
    >>> request.tag, request.is_dry_run
    ('v1.0.0', True)
    """

    tag: str
    payload_dir: pathlib.Path
    digest_record: pathlib.Path
    plan_path: pathlib.Path
    is_dry_run: bool


def write_plan(
    plan_path: pathlib.Path,
    actions: cabc.Iterable[tuple[str, release_reconcile_github.AssetAction]],
    conflicts: cabc.Iterable[str],
) -> None:
    """Write the per-asset plan in the shape the release jobs already read.

    Parameters
    ----------
    plan_path
        Destination for the JSON plan.
    actions
        One ``(name, action)`` pair per declared asset.
    conflicts
        The names whose attached bytes disagree with the trusted record.

    Notes
    -----
    JSON is sorted and newline-terminated so identical decisions serialize
    identically, which lets a job diff two runs' plans. An unwritable
    destination propagates ``OSError`` to the caller, which reports it as a
    distinct exit status rather than as an upload failure.

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
      "conflicts": []
    }
    <BLANKLINE>
    """
    plan_path.write_text(
        json.dumps(
            {
                "actions": [[name, action.value] for name, action in actions],
                "conflicts": list(conflicts),
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
