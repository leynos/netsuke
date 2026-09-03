"""Hold the cache write policy: one writer per key, trunk pushes only.

Pull requests read the trusted generation and must not publish a competing
one. Exactly one job writes each key family, and only on a push to `main`
where that key's restore missed. These checks pin that policy so an edit
cannot let a pull request race the designated writer.

Run via ``make test-workflow-contracts``.
"""

import pytest
from cache_contract_data import (
    ACTION_DIR,
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


@pytest.mark.parametrize(
    ("workflow_name", "job_name"), [("netsukefile-test.yml", "netsukefile")]
)
def test_workflow_saves_name_the_trunk_push_directly(
    workflow_name: str, job_name: str
) -> None:
    """Require every inline cache save to name a push on the trunk."""
    steps = job_steps(load_workflow(WORKFLOW_DIR / workflow_name), job_name)
    saves = [
        step for step in cache_steps(steps) if "/save@" in str(step.get("uses", ""))
    ]
    assert saves, f"{workflow_name} {job_name} is a writer and must save"
    for step in saves:
        condition = str(step.get("if", ""))
        assert is_trunk_only_save(condition), (
            f"{workflow_name} step {step.get('name')!r} must save only on a "
            f"push to main, got {condition!r}"
        )
        assert "cache-hit != 'true'" in " ".join(condition.split()), (
            f"{workflow_name} step {step.get('name')!r} must skip a save when "
            "the restore already hit that exact key"
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
        assert "-hit'] != 'true'" in condition, (
            f"{action_name} step {step.get('name')!r} must skip a save when "
            "the restore already hit that exact key"
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
