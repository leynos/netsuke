"""Hold the action's two retention steps to what they must preserve.

Retention is not incidental to this action: a run that refuses still has to
leave an operator the evidence of what it declared and what it planned, and the
upload step defers its failure precisely so that evidence survives. These cases
read the manifest's own declarations rather than a copy of them, so a step that
was reordered or renamed fails here rather than passing on a stale expectation.

Run via ``make test-workflow-contracts``.
"""

import pathlib

import pytest
from action_run_sources import action_steps, require_action_step
from upload_declared_assets_manifest import (
    ACTION,
    PLAN_STEP,
    RETAIN_STEP,
    RUN_STEPS,
    action_document,
)
from workflow_loading import require_mapping


def _basename(argument: str) -> str:
    """Return the last path segment of one recorded path argument.

    Returns
    -------
    str
        The file name, so an expectation names a record rather than the
        absolute directory a runner happened to use.
    """
    return pathlib.PurePosixPath(argument).name


def _retained_names(step: dict[str, object]) -> set[str]:
    """Return the file names one retention step uploads.

    Returns
    -------
    set[str]
        The retained file names, with blank lines dropped so a trailing newline
        in the YAML block does not read as an unnamed file.
    """
    with_block = require_mapping(step.get("with"), "a retention step's with")
    return {
        _basename(line.strip())
        for line in str(with_block["path"]).splitlines()
        if line.strip()
    }


def _retained_paths(with_block: dict[str, object]) -> list[pathlib.PurePosixPath]:
    """Return each path a retention step names, in the order it names them.

    Returns
    -------
    list[pathlib.PurePosixPath]
        The retained paths, with blank lines dropped so a trailing newline in
        the YAML block does not read as an unnamed file.
    """
    return [
        pathlib.PurePosixPath(line.strip())
        for line in str(with_block["path"]).splitlines()
        if line.strip()
    ]


def _assert_retained_layout(paths: list[pathlib.PurePosixPath]) -> None:
    """Require the retained files to sit directly in one artefact root.

    ``upload-artifact`` stores every file relative to the deepest directory its
    paths share, and the verifier downloads that artefact into ``trusted``
    before reading ``trusted/declaration.txt`` and ``trusted/trusted.json``.
    Naming one retained file from a different directory lifts that shared
    root, nesting the two files the verifier reads under an extra segment, so
    every retained path has to share one parent.
    """
    roots = {path.parent for path in paths}
    assert len(roots) == 1, (
        "every retained file must share the artefact root, got "
        f"{sorted(str(root) for root in roots)}"
    )
    # The declaration travels with both digest records: one from the selection
    # and one from the declaration step, so verification can compare them.
    names = {path.name for path in paths}
    assert names == {"declaration.txt", "selection.json", "trusted.json"}, names


def test_the_declared_record_is_retained_before_the_upload() -> None:
    """Retain the declaration and both digest records, failing if any is absent."""
    document = action_document()
    steps = action_steps(ACTION, document)
    retain_index = require_action_step(ACTION, document, RETAIN_STEP)
    upload_index = require_action_step(ACTION, document, RUN_STEPS[-1])
    # The test's own name is an ordering claim, so it checks the order.
    # Retention that slipped after the upload would still name the right files,
    # and would then retain nothing for exactly the refused runs the record
    # exists for.
    assert retain_index < upload_index, (
        "the declared record must be retained before the step that may refuse"
    )
    retain = steps[retain_index]
    with_block = require_mapping(retain.get("with"), f"{ACTION} {RETAIN_STEP} with")
    _assert_retained_layout(_retained_paths(with_block))
    assert with_block["if-no-files-found"] == "error", (
        "a missing record must fail the step rather than silently upload"
    )


def test_a_record_retained_outside_the_artefact_root_is_rejected() -> None:
    """Prove the layout contract notices a retain path in another directory.

    The unmutated manifest keeps all three files in one directory, so only a
    case that moves one of them shows whether the shared-root assertion is
    live. Moving ``selection.json`` into a subdirectory is the smallest such
    change: it looks harmless in the manifest and still breaks the verifier's
    ``trusted/...`` reads.
    """
    document = action_document()
    steps = action_steps(ACTION, document)
    retain = steps[require_action_step(ACTION, document, RETAIN_STEP)]
    with_block = require_mapping(retain.get("with"), f"{ACTION} {RETAIN_STEP} with")
    moved = [
        path.parent / "selection" / path.name if path.name == "selection.json" else path
        for path in _retained_paths(with_block)
    ]
    with pytest.raises(AssertionError):
        _assert_retained_layout(moved)


def test_the_upload_plan_is_retained_after_the_upload() -> None:
    """Keep the plan and its measurements for a refusing run, without masking it."""
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
    # `always()` is the whole point: a refused run is exactly when an operator
    # needs the plan, and a default `if:` would skip it on failure.
    assert plan.get("if") == "always()", (
        "the plan must be retained even when the upload was refused"
    )
    # The timings and the per-call trace ride with the plan: the uploader
    # writes all three beside one another, a boundary duration is only readable
    # against the run that measured it, and only the trace can say which of
    # several calls to one boundary failed.
    assert _retained_names(plan) == {
        "upload-plan.json",
        "upload-timings.json",
        "upload-trace.jsonl",
    }, "the plan, its timings, and its per-call trace must ride together"
    with_block = require_mapping(plan.get("with"), f"{ACTION} {PLAN_STEP} with")
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
