"""Hold the cache write policy: one writer per key, trunk pushes only.

Pull requests read the trusted generation and must not publish a competing
one. Exactly one job writes each key family, and only on a push to `main`
where that key's restore missed. These checks pin that policy so an edit
cannot let a pull request race the designated writer.

Run via ``make test-workflow-contracts``.
"""

import re

import pytest
from cache_contract_data import (
    ACTION_DIR,
    INLINE_SAVE_WRITERS,
    KEY_WRITERS,
    READ_ONLY_CACHE_JOBS,
    SMOKE_PROFILE_JOBS,
    WORKFLOW_DIR,
    cache_steps,
    lane_steps,
)
from runner_placement_invariants import is_trunk_only_save
from workflow_loading import (
    job_steps,
    load_workflow,
    require_mapping,
)

KEY_OUTPUT = re.compile(r"steps\.keys\.outputs(?:\.(\w+)|\['([^']+)'\])")


def _save_key_name(step: dict[str, object]) -> str:
    """Return the `keys` output name a composite save step publishes."""
    key = str(require_mapping(step.get("with"), "cache step inputs").get("key", ""))
    match = KEY_OUTPUT.search(key)
    assert match, f"save step {step.get('name')!r} must key on a rendered output"
    return match.group(1) or match.group(2)


def _restore_ids_by_key(steps: list[dict[str, object]]) -> dict[str, str]:
    """Map each restore step's rendered key to that step's id."""
    ids: dict[str, str] = {}
    for step in steps:
        if "/restore@" not in str(step.get("uses", "")):
            continue
        inputs = require_mapping(step.get("with"), "cache step inputs")
        step_id = str(step.get("id", ""))
        assert step_id, (
            f"restore step {step.get('name')!r} must declare an id, or no save "
            "can gate on its result"
        )
        ids[str(inputs.get("key", "")).strip()] = step_id
    return ids


@pytest.mark.parametrize(("workflow_name", "job_name"), INLINE_SAVE_WRITERS)
def test_workflow_saves_name_the_trunk_push_directly(
    workflow_name: str, job_name: str
) -> None:
    """Require every inline cache save to name a push on the trunk.

    The parametrization is derived from `KEY_WRITERS` rather than listed here,
    so a writer added to that table cannot gain a save step this policy never
    reads. The hit gate is bound to the save's own key: a save that skipped on
    a *different* key's restore result would either overwrite a warm archive
    or never publish one, which is exactly what this policy exists to prevent.
    """
    steps = job_steps(load_workflow(WORKFLOW_DIR / workflow_name), job_name)
    restore_ids = _restore_ids_by_key(cache_steps(steps))
    saves = [
        step for step in cache_steps(steps) if "/save@" in str(step.get("uses", ""))
    ]
    assert saves, f"{workflow_name} {job_name} is a writer and must save"
    for step in saves:
        condition = " ".join(str(step.get("if", "")).split())
        assert is_trunk_only_save(condition), (
            f"{workflow_name} step {step.get('name')!r} must save only on a "
            f"push to main, got {condition!r}"
        )
        key = str(
            require_mapping(step.get("with"), "cache step inputs").get("key", "")
        ).strip()
        step_id = restore_ids.get(key)
        assert step_id, (
            f"{workflow_name} step {step.get('name')!r} saves key {key!r}, "
            f"which no restore step in this job reads: {sorted(restore_ids)!r}"
        )
        expected = f"steps.{step_id}.outputs.cache-hit != 'true'"
        assert expected in condition, (
            f"{workflow_name} step {step.get('name')!r} must gate on its own "
            f"key's restore result ({expected}), got {condition!r}"
        )


@pytest.mark.parametrize(
    "action_name", ["linux-gate-cache", "kani-cache", "windows-gate-cache"]
)
def test_composite_saves_derive_their_gate_from_the_trunk_push(
    action_name: str,
) -> None:
    """Require a composite's saves to gate on a trunk-derived writer flag.

    The composite renders one `writer` output from the trunk condition and
    every save reads it, so the policy is stated once rather than repeated at
    each call site where it could drift.
    """
    steps = lane_steps(ACTION_DIR / action_name / "action.yml", None)
    key_step = next(step for step in steps if step.get("id") == "keys")
    trunk = str(require_mapping(key_step.get("env"), "key step env")["IS_TRUNK_PUSH"])
    assert is_trunk_only_save(trunk), (
        f"{action_name} must derive its writer flag from a trunk push, got {trunk!r}"
    )
    assert "printf 'writer=%s\\n'" in str(key_step.get("run", "")), (
        f"{action_name} must publish the writer flag as a step output"
    )
    saves = [
        step for step in cache_steps(steps) if "/save@" in str(step.get("uses", ""))
    ]
    assert saves, f"{action_name} must declare at least one save step"
    for step in saves:
        condition = " ".join(str(step.get("if", "")).split())
        assert "steps.keys.outputs.writer == 'true'" in condition, (
            f"{action_name} step {step.get('name')!r} must gate on the writer "
            f"flag, got {condition!r}"
        )
        expected = f"inputs['{_save_key_name(step)}-hit'] != 'true'"
        assert expected in condition, (
            f"{action_name} step {step.get('name')!r} must gate on the hit "
            f"input for the key it publishes ({expected}); a save gated on a "
            f"sibling key's result would either overwrite a warm archive or "
            f"never publish one. Got {condition!r}"
        )


@pytest.mark.parametrize(("workflow_name", "job_name"), READ_ONLY_CACHE_JOBS)
def test_reader_jobs_never_publish_a_generation(
    workflow_name: str, job_name: str
) -> None:
    """Keep every non-writer job to restores only.

    Exactly one job writes each key family. A reader that saved would race the
    designated writer for the reservation and publish state the writer never
    reviewed.
    """
    assert (workflow_name, job_name) not in KEY_WRITERS.values(), (
        f"{workflow_name} {job_name} is listed both as a writer and a reader"
    )
    steps = job_steps(load_workflow(WORKFLOW_DIR / workflow_name), job_name)
    saves = [
        step for step in cache_steps(steps) if "/save@" in str(step.get("uses", ""))
    ]
    assert not saves, (
        f"{workflow_name} {job_name} restores only, but declares {saves!r}"
    )


@pytest.mark.parametrize(("workflow_name", "job_name"), SMOKE_PROFILE_JOBS)
def test_native_smoke_jobs_use_the_read_only_cache_profile(
    workflow_name: str, job_name: str
) -> None:
    """Keep the native Windows smoke jobs to restores only.

    `build-test-windows` is the single writer of every Windows key family. A
    smoke job that saved would race it for the reservation and publish state
    the writer never reviewed, so it asks for the `smoke` profile, which
    declares no save step at all.
    """
    steps = job_steps(load_workflow(WORKFLOW_DIR / workflow_name), job_name)
    calls = [
        step
        for step in steps
        if str(step.get("uses", "")) == "./.github/actions/windows-gate-cache"
    ]
    assert len(calls) == 1, (
        f"{workflow_name} {job_name} must call the Windows cache action once, "
        f"got {calls!r}"
    )
    inputs = require_mapping(calls[0].get("with"), "cache action inputs")
    assert inputs.get("mode") == "restore", (
        f"{workflow_name} {job_name} must only restore"
    )
    assert inputs.get("profile") == "smoke", (
        f"{workflow_name} {job_name} must use the read-only cache profile"
    )


def _workflows_accepting_a_dispatch() -> list[str]:
    """Return every workflow this repository can start by hand."""
    names = []
    for path in sorted({*WORKFLOW_DIR.glob("*.yml"), *WORKFLOW_DIR.glob("*.yaml")}):
        triggers = load_workflow(path).get("on")
        if isinstance(triggers, dict) and "workflow_dispatch" in triggers:
            names.append(path.name)
    return names


def test_a_dispatch_can_never_publish_a_cache_generation() -> None:
    """Require every save to name a push, so a warm-run dispatch only reads.

    The exit gate for the runner migration measures warm behaviour by
    dispatching the gate workflows on `main`. That is safe only while no save
    can fire on a `workflow_dispatch` event: a dispatch that published would
    write a generation from a tree nobody reviewed, and would do it while
    racing the designated writer.

    Every save is therefore required to name the push event explicitly, in the
    composite actions' writer flag and in the inline saves alike. Naming the
    ref alone would not do: `github.ref` is `refs/heads/main` on a dispatch
    against the trunk too.
    """
    dispatchable = _workflows_accepting_a_dispatch()
    assert dispatchable, (
        "no workflow accepts a dispatch, so the exit gate cannot take warm "
        "measurements; this contract is guarding nothing"
    )
    conditions: list[tuple[str, str]] = []
    for action_name in sorted(path.name for path in ACTION_DIR.iterdir()):
        action = ACTION_DIR / action_name / "action.yml"
        if not action.is_file():
            continue
        steps = lane_steps(action, None)
        if not any("/save@" in str(step.get("uses", "")) for step in steps):
            continue
        key_step = next(step for step in steps if step.get("id") == "keys")
        env = require_mapping(key_step.get("env"), "key step env")
        conditions.append((action_name, str(env["IS_TRUNK_PUSH"])))
    for workflow_name, job_name in INLINE_SAVE_WRITERS:
        steps = job_steps(load_workflow(WORKFLOW_DIR / workflow_name), job_name)
        conditions.extend(
            (f"{workflow_name} {step.get('name')!r}", str(step.get("if", "")))
            for step in cache_steps(steps)
            if "/save@" in str(step.get("uses", ""))
        )
    assert conditions, "no cache save was found to check"
    for source, condition in conditions:
        normalized = " ".join(condition.split())
        assert "github.event_name == 'push'" in normalized, (
            f"{source} must name the push event, or a dispatch on main would "
            f"publish a generation. Got {condition!r}"
        )
