"""Execute the declared-asset action against a complete release.

Each case here runs the action's real shell bodies, through the real release
scripts, over a staged tree built from this repository's own release
configuration. The harness that stubs ``uv`` and ``gh`` and reads the manifest
lives in ``upload_declared_assets_harness.py``; the tree itself is built by
``upload_declared_assets_staging.py``.

Read together with those modules, the cases hold three properties to account:
only the declared names reach the upload, an attached asset with identical
bytes is left alone, and an attached asset whose bytes cannot be established
blocks the transfer. The cases that damage the tree and hold the refusal to
selection live in ``upload_declared_assets_damage_test.py``.

Run via ``make test-workflow-contracts``.
"""

import hashlib
import json
import typing as typ

from composite_action_execution import interpreter_baseline
from upload_declared_assets_harness import Release, execute
from upload_declared_assets_manifest import (
    RUN_STEPS,
    release_tag,
)
from upload_declared_assets_run_view import (
    basename,
    read_record,
    recorded_uploads,
    stored_plan,
)
from upload_declared_assets_staging import (
    DIST,
    declared_names,
    recorded_digests,
    release_facts,
    stage_workspace,
)

if typ.TYPE_CHECKING:
    from pathlib import Path

    from upload_declared_assets_harness import ActionRun


def _require_payload_matches_records(run: ActionRun, expected: tuple[str, ...]) -> None:
    """Assert the payload, and both digest records, describe the same bytes."""
    trusted = read_record(run, "trusted.json")
    selection = read_record(run, "selection.json")
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
    uploads = recorded_uploads(run)
    assert len(uploads) == 1, run.gh_log
    # `--clobber` would replace an attached asset rather than refuse it, and
    # the whole point of the reconciliation is that a release never does that.
    assert "--clobber" not in run.gh_log, "the uploader must never clobber"
    arguments = uploads[0]
    assert arguments[2] == release_tag(facts), arguments[2]
    assert {basename(argument) for argument in arguments[3:]} == set(expected), (
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
    run = execute(tmp_path, workspace, facts, release=Release(attached=tuple(attached)))
    message = run.outputs.get("error-message")
    assert run.outputs.get("upload-error") == "false", message
    assert not recorded_uploads(run), run.gh_log
    assert {action for _name, action in stored_plan(run)["actions"]} == {
        "skip-verified"
    }, "every identical attachment must be skipped, not re-uploaded"


def test_a_conflicting_attachment_blocks_every_upload(tmp_path: Path) -> None:
    """Refuse the release, name the conflict, and send no transfer at all."""
    facts = release_facts()
    workspace = stage_workspace(tmp_path, facts)
    offender = min(recorded_digests(workspace))
    attached = [{"name": offender, "digest": f"sha256:{'0' * 64}"}]
    run = execute(tmp_path, workspace, facts, release=Release(attached=tuple(attached)))
    assert all(result.returncode == 0 for result in run.completed), run.gh_log
    assert run.outputs.get("upload-error") == "true", (
        "a conflict must be reported as a deferred upload error"
    )
    message = run.outputs.get("error-message", "")
    assert offender in message, message
    assert "disagree with the trusted record" in message, message
    assert not recorded_uploads(run), run.gh_log
    # The plan is written even though the upload is refused, so the operator
    # can see which asset the release disagreed over.
    assert stored_plan(run)["conflicts"] == [offender], (
        "the plan must name the conflict"
    )
    assert stored_plan(run)["unverifiable"] == [], (
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
    run = execute(tmp_path, workspace, facts, release=Release(attached=tuple(attached)))
    assert run.outputs.get("upload-error") == "true", (
        "an unverifiable asset must block the upload"
    )
    message = run.outputs.get("error-message", "")
    assert offender in message, message
    assert "could not be verified" in message, message
    plan = stored_plan(run)
    assert plan["unverifiable"] == [offender], plan
    assert plan["unverifiable_reasons"] == {offender: "download-failed"}, (
        "the plan must keep the reason the digest could not be established, "
        f"got {plan['unverifiable_reasons']!r}"
    )
    assert plan["conflicts"] == [], (
        "an asset nobody could read must not be reported as a conflict"
    )
    assert not recorded_uploads(run), run.gh_log


def _disagree_with_the_declaration(step: int, records: Path) -> None:
    """Rewrite selection's record so it names bytes the declaration does not.

    Runs after the declaration step and before the upload step, which is the
    only window in which this damage is possible: both records are written
    from the same tree by different steps, and the uploader is what compares
    them.
    """
    if step != RUN_STEPS.index(RUN_STEPS[-1]):
        return
    path = records / "selection.json"
    record = json.loads(path.read_text(encoding="utf-8"))
    offender = min(record["assets"])
    record["assets"][offender] = "0" * 64
    path.write_text(json.dumps(record), encoding="utf-8")


def test_a_selection_record_disagreeing_with_the_declaration_is_refused(
    tmp_path: Path,
) -> None:
    """Stop before the network when the two records name different bytes.

    The records are the account the uploader compares the payload against, and
    each is produced from the staged tree by a different step. If they
    disagree, at least one of them does not describe this release, so nothing
    about the payload can be established -- and a run that contacted GitHub
    anyway would have already told it the release exists.
    """
    facts = release_facts()
    workspace = stage_workspace(tmp_path, facts)
    run = execute(
        tmp_path,
        workspace,
        facts,
        release=Release(before_step=_disagree_with_the_declaration),
    )
    assert run.outputs.get("upload-error") == "true", (
        "a record disagreement must be reported as a deferred upload error"
    )
    message = run.outputs.get("error-message", "")
    assert (
        "the selection and declaration records describe different bytes" in message
    ), message
    assert not run.gh_log, (
        "a run that cannot account for its bytes must not reach GitHub"
    )
    assert not recorded_uploads(run), run.gh_log
