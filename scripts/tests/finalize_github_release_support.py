"""Build the records and API answers the finalizer's suites share.

The adapter's suites — one for the read-only commands, two for the publishing
command — need the same trusted records, the same ``gh`` response shape, and
the same way of driving the command line. Keeping those here means the
response a test mocks is built by the same code path that names the
invocation, so the two cannot drift into describing different releases.
"""

import dataclasses
import json
import typing as typ

import finalize_github_release as release_script

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import pathlib

    from cmd_mox import CmdMox, CommandDouble, Invocation

TAG = "v1.2.3"
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
    """Return the candidate identity the package stage records."""
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
    """Return a reconciliation plan in the shape the planner produces."""
    return {
        "actions": [[ARCHIVE, action], *[list(row) for row in extra_actions]],
        "conflicts": list(conflicts),
        "unverifiable": list(unverifiable),
    }


def _write_json_record(
    directory: pathlib.Path, name: str, record: cabc.Mapping[str, object]
) -> pathlib.Path:
    """Write one JSON record into the run's directory and return its path."""
    path = directory / name
    _ = path.write_text(json.dumps(record), encoding="utf-8")
    return path


def write_candidate(
    directory: pathlib.Path, *, identity: cabc.Mapping[str, object] | None = None
) -> pathlib.Path:
    """Write the candidate record a publishing run reads."""
    record = identity_record() if identity is None else identity
    return _write_json_record(directory, "candidate.json", record)


def write_plan(
    directory: pathlib.Path, *, plan: cabc.Mapping[str, object] | None = None
) -> pathlib.Path:
    """Write a reconciliation plan, defaulting to one fully verified asset."""
    record = plan_record() if plan is None else plan
    return _write_json_record(directory, "plan.json", record)


def digest_record(
    assets: cabc.Mapping[str, str] | None = None,
) -> dict[str, object]:
    """Return the trusted digest record the declaration stage writes."""
    declared = {ARCHIVE: DIGEST} if assets is None else dict(assets)
    return {"assets": declared}


def write_digest_record(
    directory: pathlib.Path,
    *,
    record: cabc.Mapping[str, object] | None = None,
) -> pathlib.Path:
    """Write the trusted digest record a publishing run reconciles against."""
    written = digest_record() if record is None else record
    return _write_json_record(directory, "trusted.json", written)


def release_view(
    *,
    is_draft: bool,
    is_prerelease: bool,
    tag: str = TAG,
    assets: cabc.Mapping[str, str | None] | None = None,
) -> str:
    """Return the JSON ``gh release view`` prints for one release state."""
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


def publish_arguments(
    directory: pathlib.Path,
    records: PublishRecords | None = None,
    *,
    github_output: pathlib.Path | None = None,
) -> list[str]:
    """Return a complete ``publish`` invocation over freshly written inputs."""
    records = PublishRecords() if records is None else records
    record = identity_record() if records.candidate is None else records.candidate
    candidate_path = write_candidate(directory, identity=record)
    plan_path = write_plan(directory, plan=records.plan)
    digest_path = write_digest_record(directory, record=records.trusted)
    arguments = [
        "publish",
        "--tag",
        str(record["tag"]),
        "--commit",
        COMMIT,
        "--repository",
        REPOSITORY,
        "--candidate-path",
        str(candidate_path),
        "--plan-path",
        str(plan_path),
        "--digest-record-path",
        str(digest_path),
        "--result-path",
        str(directory / RESULT_NAME),
    ]
    if github_output is not None:
        arguments.extend(["--github-output", str(github_output)])
    return arguments


def run_finalizer(*arguments: str) -> None:
    """Invoke the adapter as the workflow step would."""
    release_script.app(list(arguments), result_action="return_value")


def edit_arguments(tag: str, *, is_prerelease: bool) -> tuple[str, ...]:
    """Return the one edit the finalizer is allowed to make."""
    flag = "true" if is_prerelease else "false"
    return (
        "release",
        "edit",
        tag,
        "--repo",
        REPOSITORY,
        "--draft=false",
        f"--prerelease={flag}",
    )


def invocation_arguments(invocation: Invocation) -> tuple[str, ...]:
    """Return an invocation's arguments as a hashable sequence."""
    return tuple(invocation.args)


def sequenced_gh(
    cmd_mox: CmdMox, responses: cabc.Sequence[tuple[str, str, int]]
) -> CommandDouble:
    """Mock ``gh`` to answer with each response in turn."""
    remaining = list(responses)

    def respond(invocation: Invocation) -> tuple[str, str, int]:
        """Answer the next call with the next recorded response."""
        del invocation
        return remaining.pop(0)

    return cmd_mox.mock("gh").runs(respond).times(len(responses))


def release_views(
    *states: tuple[bool, bool], tag: str = TAG
) -> list[tuple[str, str, int]]:
    """Return successful ``gh release view`` responses for each state."""
    return [
        (
            release_view(is_draft=is_draft, is_prerelease=is_prerelease, tag=tag),
            "",
            0,
        )
        for is_draft, is_prerelease in states
    ]


def result_record(directory: pathlib.Path) -> object:
    """Read the outcome record the publish command wrote."""
    return json.loads((directory / "result.json").read_text(encoding="utf-8"))
