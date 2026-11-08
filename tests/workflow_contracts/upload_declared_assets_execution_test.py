"""Execute the declared-asset action against a complete, and a damaged, release.

Each case here runs the action's real shell bodies, through the real release
scripts, over a staged tree built from this repository's own release
configuration. The harness that stubs ``uv`` and ``gh`` and reads the manifest
lives in ``upload_declared_assets_harness.py``; the tree itself is built by
``upload_declared_assets_staging.py``.

Read together with those modules, the cases hold four properties to account:
only the declared names reach the upload, an attached asset with identical
bytes is left alone, an attached asset with different bytes blocks every
transfer, and any disagreement inside the staged tree stops at selection --
before the network is touched at all.

Run via ``make test-workflow-contracts``.
"""

import hashlib
import json
import pathlib
import shutil
import typing as typ

import pytest
from action_run_sources import action_steps, require_action_step
from upload_declared_assets_harness import execute, interpreter_baseline
from upload_declared_assets_manifest import (
    ACTION,
    PLAN_STEP,
    RETAIN_STEP,
    RUN_STEPS,
    action_document,
    release_tag,
)
from upload_declared_assets_staging import (
    DIST,
    Facts,
    archive_name,
    declared_names,
    package_names,
    record_path,
    recorded_digests,
    release_facts,
    rewrite_record,
    stage_workspace,
)
from workflow_loading import require_mapping

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    from pathlib import Path

    from upload_declared_assets_harness import ActionRun

#: Each ``gh`` line records the arguments it was given, so an upload's asset
#: names are the last path segments of those arguments.
_UPLOAD_PREFIX = "release upload"


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
    """

    actions: list[list[str]]
    conflicts: list[str]
    unverifiable: list[str]


class DigestRecord(typ.TypedDict):
    """Describe one retained digest record.

    Attributes
    ----------
    assets
        Published name mapped to the SHA-256 of the bytes the record covers.
    """

    assets: dict[str, str]


def _basename(argument: str) -> str:
    """Return the last path segment of one recorded path argument.

    The recorded arguments are POSIX paths from the runner, so the segment is
    taken with a pure POSIX path rather than by splitting the string.

    Returns
    -------
    str
        The segment ``gh`` would name the asset from.
    """
    return pathlib.PurePosixPath(argument).name


def _uploads(run: ActionRun) -> list[list[str]]:
    """Return each recorded upload as its list of arguments.

    Returns
    -------
    list[list[str]]
        One argument list per ``gh release upload`` invocation.
    """
    return [
        line.split()
        for line in run.gh_log.splitlines()
        if line.startswith(_UPLOAD_PREFIX)
    ]


def _plan(run: ActionRun) -> UploadPlan:
    """Return the upload plan the uploader wrote.

    Returns
    -------
    UploadPlan
        The decoded plan, holding ``actions``, ``conflicts``, and
        ``unverifiable``.
    """
    path = run.declared_root / "upload-plan.json"
    decoded: UploadPlan = json.loads(path.read_text(encoding="utf-8"))
    return decoded


def _read_record(run: ActionRun, name: str) -> DigestRecord:
    """Return one decoded digest record the action retained.

    Returns
    -------
    DigestRecord
        The decoded record, holding the asset digests under ``assets``.
    """
    path = run.declared_root / name
    decoded: DigestRecord = json.loads(path.read_text(encoding="utf-8"))
    return decoded


def _require_payload_matches_records(run: ActionRun, expected: tuple[str, ...]) -> None:
    """Assert the payload, and both digest records, describe the same bytes."""
    trusted = _read_record(run, "trusted.json")
    selection = _read_record(run, "selection.json")
    # The two records are produced from the same tree by different steps, so
    # agreement is evidence the payload did not change between them.
    assert trusted["assets"] == selection["assets"], (
        "the trusted and selected records must describe the same bytes"
    )
    for name in expected:
        digest = hashlib.sha256((run.payload / name).read_bytes()).hexdigest()
        assert trusted["assets"][name] == digest, name


def test_the_steps_upload_exactly_the_declared_assets(tmp_path: Path) -> None:
    """Carry a complete staged tree through selection, declaration, and upload."""
    facts = release_facts()
    workspace = stage_workspace(tmp_path, facts)
    run = execute(tmp_path, workspace, facts)
    expected = declared_names(facts)
    assert len(run.completed) == len(RUN_STEPS), run.completed[-1].stderr
    message = run.outputs.get("error-message")
    assert run.outputs.get("upload-error") == "false", message
    staged = tuple(sorted(path.name for path in run.payload.iterdir()))
    assert staged == expected, "the payload must hold exactly the declared names"
    # The retained files a checkout leaves beside the artefacts must survive:
    # they are neither declared nor hashed, but selection must not move them.
    assert (workspace / DIST / facts.targets[0].key / "netsuke").is_file(), (
        "selection must not consume the retained files a checkout leaves"
    )
    declaration = (run.declared_root / "declaration.txt").read_text(encoding="utf-8")
    assert tuple(declaration.splitlines()) == expected, (
        "the declaration must list every declared name, once"
    )
    _require_payload_matches_records(run, expected)
    uploads = _uploads(run)
    assert len(uploads) == 1, run.gh_log
    # `--clobber` would replace an attached asset rather than refuse it, and
    # the whole point of the reconciliation is that a release never does that.
    assert "--clobber" not in run.gh_log, "the uploader must never clobber"
    arguments = uploads[0]
    assert arguments[2] == release_tag(facts), arguments[2]
    assert {_basename(argument) for argument in arguments[3:]} == set(expected), (
        "the transferred asset names must be the declared set"
    )
    assert set(run.baselines) == {interpreter_baseline()}, run.baselines


def test_an_identical_attached_asset_is_left_alone(tmp_path: Path) -> None:
    """Skip every name whose attached bytes already match the trusted record."""
    facts = release_facts()
    workspace = stage_workspace(tmp_path, facts)
    recorded = sorted(recorded_digests(workspace).items())
    attached = [
        {"name": name, "digest": f"sha256:{digest}"} for name, digest in recorded
    ]
    run = execute(tmp_path, workspace, facts, attached=attached)
    message = run.outputs.get("error-message")
    assert run.outputs.get("upload-error") == "false", message
    assert not _uploads(run), run.gh_log
    assert {action for _name, action in _plan(run)["actions"]} == {"skip-verified"}, (
        "every identical attachment must be skipped, not re-uploaded"
    )


def test_a_conflicting_attachment_blocks_every_upload(tmp_path: Path) -> None:
    """Refuse the release, name the conflict, and send no transfer at all."""
    facts = release_facts()
    workspace = stage_workspace(tmp_path, facts)
    offender = min(recorded_digests(workspace))
    attached = [{"name": offender, "digest": f"sha256:{'0' * 64}"}]
    run = execute(tmp_path, workspace, facts, attached=attached)
    assert all(result.returncode == 0 for result in run.completed), run.gh_log
    assert run.outputs.get("upload-error") == "true", (
        "a conflict must be reported as a deferred upload error"
    )
    message = run.outputs.get("error-message", "")
    assert offender in message, message
    assert "disagree with the trusted record" in message, message
    assert not _uploads(run), run.gh_log
    # The plan is written even though the upload is refused, so the operator
    # can see which asset the release disagreed over.
    assert _plan(run)["conflicts"] == [offender], "the plan must name the conflict"
    assert _plan(run)["unverifiable"] == [], (
        "bytes that were read and disagreed are not an unreadable asset"
    )


def test_an_asset_github_cannot_vouch_for_is_reported_as_unverifiable(
    tmp_path: Path,
) -> None:
    """Keep "could not tell" apart from "holds different bytes".

    GitHub may list an attached asset with no usable digest, and the only
    remaining evidence is a download. When that download fails, the run knows
    nothing about the bytes -- which stops the upload just as a conflict does,
    but calls for a different remedy. The plan must record the two under their
    own keys, or an operator is sent hunting for wrong bytes that may not
    exist.
    """
    facts = release_facts()
    workspace = stage_workspace(tmp_path, facts)
    offender = min(recorded_digests(workspace))
    # A digest the adapter cannot normalise, over a download the stub refuses:
    # the listing names the asset and nothing establishes its bytes.
    attached = [{"name": offender, "digest": "md5:00000000000000000000000000000000"}]
    run = execute(tmp_path, workspace, facts, attached=attached)
    assert run.outputs.get("upload-error") == "true", (
        "an unverifiable asset must block the upload"
    )
    message = run.outputs.get("error-message", "")
    assert offender in message, message
    assert "could not be verified" in message, message
    plan = _plan(run)
    assert plan["unverifiable"] == [offender], plan
    assert plan["conflicts"] == [], (
        "an asset nobody could read must not be reported as a conflict"
    )
    assert not _uploads(run), run.gh_log


def test_the_declared_record_is_retained_before_the_upload() -> None:
    """Retain the declaration and both digest records, failing if any is absent."""
    document = action_document()
    steps = action_steps(ACTION, document)
    retain = steps[require_action_step(ACTION, document, RETAIN_STEP)]
    with_block = require_mapping(retain.get("with"), f"{ACTION} {RETAIN_STEP} with")
    names = {
        _basename(line.strip())
        for line in str(with_block["path"]).splitlines()
        if line.strip()
    }
    # The declaration travels with both digest records: one from the selection
    # and one from the declaration step, so verification can compare them.
    assert names == {"declaration.txt", "selection.json", "trusted.json"}, names
    assert with_block["if-no-files-found"] == "error", (
        "a missing record must fail the step rather than silently upload"
    )


def test_the_upload_plan_is_retained_after_the_upload() -> None:
    """Keep the plan for a refusing run, without masking the uploader."""
    document = action_document()
    steps = action_steps(ACTION, document)
    plan_index = require_action_step(ACTION, document, PLAN_STEP)
    upload_index = require_action_step(ACTION, document, RUN_STEPS[-1])
    # The plan is written by the upload step, so its retention has to come
    # after it. A retention declared earlier would upload nothing.
    assert plan_index > upload_index, (
        "the plan must be retained after the step that writes it"
    )
    plan = steps[plan_index]
    with_block = require_mapping(plan.get("with"), f"{ACTION} {PLAN_STEP} with")
    # `always()` is the whole point: a refused run is exactly when an operator
    # needs the plan, and a default `if:` would skip it on failure.
    assert plan.get("if") == "always()", (
        "the plan must be retained even when the upload was refused"
    )
    assert _basename(str(with_block["path"]).strip()) == "upload-plan.json", (
        "the retained file must be the plan the uploader wrote"
    )
    # `warn`, not `error`: a failure before the uploader runs leaves no plan,
    # and that must not replace the uploader's diagnostic with a second one.
    assert with_block["if-no-files-found"] == "warn", (
        "a missing plan must not mask the uploader's own diagnostic"
    )


@pytest.mark.parametrize("step_name", [RETAIN_STEP, PLAN_STEP])
def test_a_job_rerun_retains_its_own_artefact(step_name: str) -> None:
    """Keep each attempt's evidence apart instead of colliding or replacing it.

    ``upload-artifact`` v7 refuses to overwrite an existing artefact and a job
    re-run keeps the run id, so a name without the attempt would fail the
    re-run's retention step. Recording ``overwrite: true`` instead would
    resolve that collision by deleting the first attempt's record, which is the
    opposite of what these steps exist to preserve.
    """
    document = action_document()
    steps = action_steps(ACTION, document)
    step = steps[require_action_step(ACTION, document, step_name)]
    with_block = require_mapping(step.get("with"), f"{ACTION} {step_name} with")
    name = str(with_block["name"]).strip()
    assert name.endswith("-${{ github.run_attempt }}"), (
        f"{step_name} must distinguish attempts, got {name!r}"
    )
    # The job name separates these artefacts from each other, and from any
    # other job that retains its own record in the same run.
    assert "${{ github.job }}" in name, f"{step_name} must name its job: {name!r}"
    assert "overwrite" not in with_block, (
        f"{step_name} must preserve an earlier attempt's artefact, not replace it"
    )


def _stale_commit(workspace: Path, facts: Facts) -> None:
    """Claim the records were built from another commit."""
    rewrite_record(workspace, facts.targets[0], commit="b" * 40)


def _foreign_run(workspace: Path, facts: Facts) -> None:
    """Claim the records were written by another workflow run."""
    rewrite_record(workspace, facts.targets[0], run_id="4243")


def _missing_target(workspace: Path, facts: Facts) -> None:
    """Remove one target's producer record entirely."""
    record_path(workspace, facts.targets[0]).unlink()


def _duplicate_target(workspace: Path, facts: Facts) -> None:
    """Make two records claim the same configured target."""
    shutil.copy2(
        record_path(workspace, facts.targets[0]),
        record_path(workspace, facts.targets[1]),
    )


def _release_shaped_stray(workspace: Path, facts: Facts) -> None:
    """Stage an archive-shaped file no producer declares."""
    stray = workspace / DIST / f"stray-{facts.version}.tar.gz"
    stray.write_bytes(b"stray\n")


def _ambiguous_asset(workspace: Path, facts: Facts) -> None:
    """Publish one archive name from two places in the tree."""
    target = facts.targets[0]
    archive = archive_name(facts, target)
    shutil.copy2(workspace / DIST / archive, workspace / DIST / target.key / archive)


def _replaced_bytes(workspace: Path, facts: Facts) -> None:
    """Replace a staged package's bytes after its producer recorded them."""
    target = facts.targets[0]
    path = workspace / DIST / target.key / package_names(facts, target)[0]
    path.write_bytes(b"rebuilt elsewhere\n")


def _symlinked_asset(workspace: Path, facts: Facts) -> None:
    """Stage one declared package as a link rather than the bytes themselves."""
    target = facts.targets[0]
    path = workspace / DIST / target.key / package_names(facts, target)[0]
    path.unlink()
    path.symlink_to(workspace / DIST / archive_name(facts, target))


#: Each way a staged tree can disagree with the release, and the diagnosis the
#: operator must be given for it. Every entry must stop at selection: the tree
#: is refused before the uploader can read the release at all.
DAMAGE: dict[str, tuple[cabc.Callable[[Path, Facts], None], str]] = {
    "ambiguous-asset": (_ambiguous_asset, "was staged 2 times"),
    "duplicate-target": (
        _duplicate_target,
        "more than one producer record covers targets",
    ),
    "foreign-run": (_foreign_run, "not the current run's"),
    "missing-target": (_missing_target, "no producer record covers targets"),
    "release-shaped-stray": (
        _release_shaped_stray,
        "is release-shaped but not declared",
    ),
    "replaced-bytes": (_replaced_bytes, "holds bytes its producer did not record"),
    "stale-candidate": (_stale_commit, "not candidate"),
    "symlinked-asset": (_symlinked_asset, "was not staged"),
}


@pytest.mark.parametrize("kind", sorted(DAMAGE))
def test_a_refused_tree_stops_before_any_network_call(
    tmp_path: Path, kind: str
) -> None:
    """Stop at selection, naming the fault, before anything is uploaded."""
    facts = release_facts()
    workspace = stage_workspace(tmp_path, facts)
    damage, diagnosis = DAMAGE[kind]
    damage(workspace, facts)
    run = execute(tmp_path, workspace, facts)
    assert len(run.completed) == 1, "selection must be the failing step"
    assert run.completed[0].returncode == 1, run.completed[0].stderr
    assert diagnosis in run.completed[0].stderr, run.completed[0].stderr
    assert not run.gh_log, "a refused tree must not reach the network"
