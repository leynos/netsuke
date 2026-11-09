"""Execute the upload step under all three values its ``dry-run`` input takes.

``dry-run`` reaches the upload step as an environment variable and is read by
the step's own ``case`` statement, which selects the flag the uploader is
called with. A ``case`` that inverted its arms would make a rehearsal upload,
or a publishing run silently upload nothing, and every structural assertion in
the manifest cases would stay green: they check which expression the ``env``
block names, not what the shell then does with it.

Each case here runs the real bodies over a complete staged tree and asserts
what the transfer did, and -- for a value the input does not admit -- that the
step refused rather than guessing a mode. The invalid arm is the one that
matters most: a step that substituted ``false`` for an unrecognised value would
upload during a rehearsal.

The tree is built by ``upload_declared_assets_staging.py`` and executed by
``upload_declared_assets_harness.py``; the mode-independent refusals live in
``upload_declared_assets_damage_test.py``.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
from upload_declared_assets_harness import ActionRun, Release, execute
from upload_declared_assets_manifest import RUN_STEPS
from upload_declared_assets_staging import release_facts, stage_workspace

if typ.TYPE_CHECKING:
    import pathlib


def _uploads(run: ActionRun) -> list[list[str]]:
    """Return each recorded upload as its argument list.

    Returns
    -------
    list[list[str]]
        One argument list per ``gh release upload`` invocation.
    """
    return [
        line.split()
        for line in run.gh_log.splitlines()
        if line.startswith("release upload")
    ]


def _mode_run(tmp_path: pathlib.Path, dry_run: str) -> ActionRun:
    """Run the action over a complete tree in one dry-run mode.

    Returns
    -------
    ActionRun
        What the steps did.
    """
    facts = release_facts()
    workspace = stage_workspace(tmp_path, facts)
    return execute(tmp_path, workspace, facts, release=Release(dry_run=dry_run))


def test_a_publishing_run_transfers_the_declared_assets(tmp_path: pathlib.Path) -> None:
    """Send the payload when ``dry-run`` is ``false``.

    ``false`` selects no flag, so the uploader's own default -- publish -- is
    what runs. The positive control for the rehearsal below: without it, a run
    that never uploaded in any mode would satisfy every assertion there.
    """
    run = _mode_run(tmp_path, "false")
    assert len(run.completed) == len(RUN_STEPS), run.completed[-1].stderr
    assert run.outputs.get("upload-error") == "false", run.outputs.get("error-message")
    uploads = _uploads(run)
    assert len(uploads) == 1, run.gh_log
    assert (run.payload / uploads[0][-1].rsplit("/", 1)[-1]).is_file(), (
        "the upload must name a file the payload really holds"
    )


def test_a_rehearsal_plans_without_transferring(tmp_path: pathlib.Path) -> None:
    """Send nothing when ``dry-run`` is ``true``, and still write the plan.

    The plan is the whole point of a rehearsal: it is what a caller inspects to
    learn what a publishing run would do. A step that skipped the uploader
    entirely would leave no plan and pass an assertion written only against
    ``gh``.
    """
    run = _mode_run(tmp_path, "true")
    assert len(run.completed) == len(RUN_STEPS), run.completed[-1].stderr
    assert run.outputs.get("upload-error") == "false", run.outputs.get("error-message")
    assert not _uploads(run), run.gh_log
    plan = (run.declared_root / "upload-plan.json").read_text(encoding="utf-8")
    assert '"upload"' in plan, (
        f"a rehearsal must still record what it would send: {plan}"
    )


@pytest.mark.parametrize("dry_run", ["", "TRUE", "yes", "1"])
def test_an_unrecognised_mode_is_refused_before_the_uploader_runs(
    tmp_path: pathlib.Path, dry_run: str
) -> None:
    """Refuse a value that is neither ``true`` nor ``false``.

    The step must stop at its own ``case`` statement rather than choosing a
    mode, because either guess is wrong: ``false`` would upload during a
    rehearsal, and ``true`` would turn a publishing run into a silent no-op.
    The run is asserted to show those two things apart: an unmatched value exits
    non-zero and no uploader is invoked, so no ``gh`` call can happen either.
    """
    run = _mode_run(tmp_path, dry_run)
    # The upload step is the last of the three script steps; selection and
    # declaration ran, then the mode was refused.
    assert len(run.completed) == len(RUN_STEPS), (
        f"{dry_run!r} must stop at the upload step, got {len(run.completed)} steps"
    )
    assert run.completed[-1].returncode != 0, run.completed[-1].stdout
    assert "dry-run must be true or false" in run.completed[-1].stderr, run.completed[
        -1
    ].stderr
    assert not _uploads(run), run.gh_log
