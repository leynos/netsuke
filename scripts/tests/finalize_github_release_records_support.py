"""Shape and write the records one finalization run reads and records.

The adapter's suites share three inputs — the candidate identity, the
reconciliation plan, and the trusted digest record — and one output, the
outcome record. Building them here, beside the ``gh`` answer shape, means a
mocked response and a written record are produced by the same description of a
release, so a suite cannot assert against one shape while the run reads another.

Nothing here drives the adapter; ``finalize_github_release_support`` owns the
command line and the mocked process.
"""

import dataclasses
import json
import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import pathlib

#: The tag the well-formed fixtures name.
TAG = "v1.2.3"

#: The full commit every fixture records. A fixed value, rather than a copy of
#: the candidate's, is what lets a test stage a candidate for another revision.
COMMIT = "0" * 40

REPOSITORY = "leynos/netsuke"
ARCHIVE = "netsuke-0.1.0-x86_64-unknown-linux-gnu.tar.gz"
SIDECAR = f"{ARCHIVE}.sha256"
DIGEST = "ab" * 32
SIDECAR_DIGEST = "cd" * 32
RESULT_NAME = "result.json"

VIEW_FIELDS = "tagName,isDraft,isPrerelease,assets"
VIEW_ARGUMENTS = (
    "release",
    "view",
    TAG,
    "--repo",
    REPOSITORY,
    "--json",
    VIEW_FIELDS,
)


def identity_record(
    *,
    is_prerelease: bool = False,
    tag: str | None = TAG,
    commit: str = COMMIT,
    version: str = "1.2.3",
) -> dict[str, object]:
    """Return the candidate identity the package stage records.

    Parameters
    ----------
    is_prerelease
        Whether the candidate publishes as a prerelease.
    tag
        The tag the candidate binds. ``None`` records one with no tag, which the
        publishing run must refuse rather than treat as this run's.
    commit
        The full revision the build was made from.
    version
        The crate version the candidate describes.

    Returns
    -------
    dict[str, object]
        The identity in the shape ``package-crate`` writes it.
    """
    return {
        "tag": tag,
        "commit": commit,
        "package_name": "netsuke-build",
        "version": version,
        "bin_name": "netsuke",
        "is_prerelease": is_prerelease,
    }


def plan_record(
    *,
    action: str = "skip-verified",
    extra_actions: cabc.Sequence[cabc.Sequence[str]] = (),
    conflicts: cabc.Sequence[str] = (),
    unverifiable: cabc.Sequence[str] = (),
) -> dict[str, object]:
    """Return a reconciliation plan in the shape the planner produces.

    Parameters
    ----------
    action
        The planner's decision for :data:`ARCHIVE`.
    extra_actions
        Further ``(name, action)`` rows, placed after the first.
    conflicts
        Asset names whose recorded bytes disagree with the release.
    unverifiable
        Asset names whose bytes the run could not establish.

    Returns
    -------
    dict[str, object]
        The plan document ``assess_plan`` reads.
    """
    return {
        "actions": [[ARCHIVE, action], *[list(row) for row in extra_actions]],
        "conflicts": list(conflicts),
        "unverifiable": list(unverifiable),
    }


def digest_record(
    assets: cabc.Mapping[str, str] | None = None,
) -> dict[str, object]:
    """Return the trusted digest record the declaration stage writes.

    Parameters
    ----------
    assets
        Map of asset name to trusted digest. Defaults to :data:`ARCHIVE` holding
        :data:`DIGEST`.

    Returns
    -------
    dict[str, object]
        The ``{"assets": ...}`` record the publishing run reconciles against.
    """
    declared = {ARCHIVE: DIGEST} if assets is None else dict(assets)
    return {"assets": declared}


def _write_json_record(
    directory: pathlib.Path, name: str, record: cabc.Mapping[str, object]
) -> pathlib.Path:
    """Write one JSON record into the run's directory and return its path."""
    path = directory / name
    _ = path.write_text(json.dumps(record), encoding="utf-8")
    return path


def _write_defaulted_record(
    directory: pathlib.Path,
    name: str,
    record: cabc.Mapping[str, object] | None,
    default: cabc.Callable[[], cabc.Mapping[str, object]],
) -> pathlib.Path:
    """Write one record, supplying the well-formed default when none is given.

    A suite names a record only to depart from the healthy one, so the default
    is what makes every other case a single departure from a run that passes.

    Returns
    -------
    pathlib.Path
        The path ``name`` resolves to inside ``directory``.
    """
    return _write_json_record(directory, name, default() if record is None else record)


def write_candidate(
    directory: pathlib.Path, *, identity: cabc.Mapping[str, object] | None = None
) -> pathlib.Path:
    """Write the candidate record a publishing run reads.

    Defaults to :func:`identity_record`'s identity, so a suite overrides
    ``identity`` only to depart from one a healthy run accepts.

    Parameters
    ----------
    directory
        Directory ``candidate.json`` is written to.
    identity
        The candidate record to write. ``None`` writes the healthy default.

    Returns
    -------
    pathlib.Path
        Where ``candidate.json`` was written.
    """
    return _write_defaulted_record(
        directory, "candidate.json", identity, identity_record
    )


def write_plan(
    directory: pathlib.Path, *, plan: cabc.Mapping[str, object] | None = None
) -> pathlib.Path:
    """Write a reconciliation plan, defaulting to one fully verified asset.

    Defaults to :func:`plan_record`'s plan, so a suite overrides ``plan`` only
    to stage the reconciliation it means to exercise.

    Parameters
    ----------
    directory
        Directory ``plan.json`` is written to.
    plan
        The plan document to write. ``None`` writes the verified default.

    Returns
    -------
    pathlib.Path
        Where ``plan.json`` was written.
    """
    return _write_defaulted_record(directory, "plan.json", plan, plan_record)


def write_digest_record(
    directory: pathlib.Path,
    *,
    record: cabc.Mapping[str, object] | None = None,
) -> pathlib.Path:
    """Write the trusted digest record a publishing run reconciles against.

    Defaults to :func:`digest_record`'s record, so a suite overrides ``record``
    only to deviate from the digests a healthy run trusts.

    Parameters
    ----------
    directory
        Directory ``trusted.json`` is written to.
    record
        The trust declaration to write. ``None`` writes the default digests.

    Returns
    -------
    pathlib.Path
        Where ``trusted.json`` was written.
    """
    return _write_defaulted_record(directory, "trusted.json", record, digest_record)


def release_view(
    *,
    is_draft: bool,
    is_prerelease: bool,
    tag: str = TAG,
    assets: cabc.Mapping[str, str | None] | None = None,
) -> str:
    """Return the JSON ``gh release view`` prints for one release state.

    Parameters
    ----------
    is_draft
        Whether ``gh`` reports the release as a draft.
    is_prerelease
        Whether ``gh`` reports it as a prerelease.
    tag
        The tag the answer names.
    assets
        Map of asset name to the digest ``gh`` reports, with ``None`` for an
        asset it gives no digest. Defaults to one archive holding ``sha256:``
        plus :data:`DIGEST`.

    Returns
    -------
    str
        The JSON text the mocked command answers with.
    """
    if assets is None:
        assets = {ARCHIVE: f"sha256:{DIGEST}"}
    entries: list[dict[str, object]] = []
    for name, digest in assets.items():
        entry: dict[str, object] = {"name": name}
        if digest is not None:
            entry["digest"] = digest
        entries.append(entry)
    return json.dumps({
        "tagName": tag,
        "isDraft": is_draft,
        "isPrerelease": is_prerelease,
        "assets": entries,
    })


def result_record(directory: pathlib.Path) -> object:
    """Read the outcome record the publish command wrote.

    Parameters
    ----------
    directory
        The run's directory, which holds ``result.json``.

    Returns
    -------
    object
        The decoded record, compared whole so an extra key is a failure.
    """
    return json.loads((directory / RESULT_NAME).read_text(encoding="utf-8"))


@dataclasses.dataclass(frozen=True, slots=True)
class PublishRecords:
    """Name the record overrides one publishing test needs.

    The three records a ``publish`` run reads are usually the well-formed
    defaults, so grouping the overrides keeps the fixture's signature about the
    run rather than about each document it happens to write.

    Attributes
    ----------
    candidate
        The candidate record to write instead of the default identity.
    plan
        The reconciliation plan to write instead of the verified default.
    trusted
        The trusted digest record to write instead of the default.
    """

    candidate: cabc.Mapping[str, object] | None = None
    plan: cabc.Mapping[str, object] | None = None
    trusted: cabc.Mapping[str, object] | None = None
