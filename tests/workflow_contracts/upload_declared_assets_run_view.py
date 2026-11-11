"""Read what one executed declared-asset run recorded.

The cases in ``upload_declared_assets_execution_test.py`` each drive a run and
then read three things back: the ``gh`` arguments it logged, the upload plan it
retained, and the digest records it wrote. Decoding those is not part of any
case's subject, so it lives here and the test module names only the field it is
asserting on.

Each reader states the shape it decodes rather than returning ``object``. The
plan and the records are contracts between the run and the job that follows it,
so a case reading ``["conflicts"]`` is checked against a mapping and not
narrowed by hand.

Run via ``make test-workflow-contracts``.
"""

import json
import pathlib
import typing as typ

if typ.TYPE_CHECKING:
    from upload_declared_assets_harness import ActionRun

#: Each ``gh`` line records the arguments it was given, so an upload's asset
#: names are the last path segments of those arguments.
UPLOAD_PREFIX = "release upload"


class UploadPlan(typ.TypedDict):
    """Describe the plan the upload step retained.

    Attributes
    ----------
    actions
        One ``[name, action]`` pair per decided asset.
    conflicts
        The names whose attached bytes disagreed with the trusted record.
    unverifiable
        The names the run could not establish anything about. Separate from
        ``conflicts`` because "the release holds different bytes" and "the
        release could not be read" call for different operator responses, and
        a plan that folded them together would report one as the other.
    unverifiable_reasons
        Unverifiable name mapped to the bounded reason the run could not read
        it, so a plan that names the blocked asset also keeps the remedy an
        operator needs to choose.
    """

    actions: list[list[str]]
    conflicts: list[str]
    unverifiable: list[str]
    unverifiable_reasons: dict[str, str]


class DigestRecord(typ.TypedDict):
    """Describe one retained digest record.

    Attributes
    ----------
    assets
        Published name mapped to the SHA-256 of the bytes the record covers.
    """

    assets: dict[str, str]


def basename(argument: str) -> str:
    """Return the last path segment of one recorded path argument.

    The recorded arguments are POSIX paths from the runner, so the segment is
    taken with a pure POSIX path rather than by splitting the string.

    Returns
    -------
    str
        The segment ``gh`` would name the asset from.
    """
    return pathlib.PurePosixPath(argument).name


def recorded_uploads(run: ActionRun) -> list[list[str]]:
    """Return each recorded upload as its list of arguments.

    Returns
    -------
    list[list[str]]
        One argument list per ``gh release upload`` invocation.
    """
    return [
        line.split()
        for line in run.gh_log.splitlines()
        if line.startswith(UPLOAD_PREFIX)
    ]


def stored_plan(run: ActionRun) -> UploadPlan:
    """Return the upload plan the uploader wrote.

    Returns
    -------
    UploadPlan
        The decoded plan, holding ``actions``, ``conflicts``,
        ``unverifiable``, and ``unverifiable_reasons``.
    """
    path = run.declared_root / "upload-plan.json"
    decoded: UploadPlan = json.loads(path.read_text(encoding="utf-8"))
    return decoded


def read_record(run: ActionRun, name: str) -> DigestRecord:
    """Return one decoded digest record the action retained.

    Returns
    -------
    DigestRecord
        The decoded record, holding the asset digests under ``assets``.
    """
    path = run.declared_root / name
    decoded: DigestRecord = json.loads(path.read_text(encoding="utf-8"))
    return decoded
