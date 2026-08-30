"""Contract tests for the mutation-testing caller workflow.

The executable logic lives in the ``leynos/shared-actions`` reusable
workflow, which carries its own unit and integration tests; netsuke's
caller is declarative configuration. These tests parse the caller through
the shared workflow loader and pin the contract it must uphold, so drift (repointing the
reference at a branch, widening permissions, or losing the Kani-module
excludes or the feature args) fails CI on the pull request rather than
surfacing in a scheduled or manual run.

The caller must reference the correct reusable workflow at a commit SHA;
Dependabot owns the SHA value, so this test asserts the shape of the pin
(a 40-character lowercase hex commit SHA) rather than a specific SHA.

Run via ``make test-workflow-contracts``.
"""

from workflow_loading import (
    MUTATION_TESTING_WORKFLOW_PATH,
    load_workflow,
    require_mapping,
)

EXPECTED_USES_PATH = "leynos/shared-actions/.github/workflows/mutation-cargo.yml"

#: Kani-only verification modules gated behind ``#[cfg(kani)]`` mod
#: declarations; cargo-mutants does not evaluate that cfg, so their
#: survivors would be noise.
KANI_EXCLUDES = (
    "src/ir/cycle_verification.rs",
    "src/ir/from_manifest_verification.rs",
    "src/ir/graph_kani_map.rs",
)

#: The exact caller configuration; assert the whole block so an added or
#: dropped input fails loudly.
EXPECTED_WITH = {
    "exclude-globs": ",".join(KANI_EXCLUDES),
    "extra-args": "--all-features",
}


def _triggers(workflow: dict[str, object]) -> dict[str, object]:
    """Return the workflow's ``on:`` mapping."""
    return require_mapping(workflow.get("on"), "the workflow on mapping")


def _mutation_job(workflow: dict[str, object]) -> dict[str, object]:
    """Return the single calling job."""
    jobs = require_mapping(workflow.get("jobs"), "the workflow jobs mapping")
    assert jobs, "the workflow must declare at least one job"
    assert list(jobs) == ["mutation"], (
        f"expected a single job named 'mutation', found {sorted(jobs)}"
    )
    return require_mapping(jobs["mutation"], "jobs.mutation")


def test_uses_reference_is_pinned_to_a_commit_sha() -> None:
    """The job must call mutation-cargo.yml, pinned to a full commit SHA.

    Dependabot owns the SHA value, so this asserts the shape of the pin
    (the correct reusable-workflow path, at a 40-character lowercase hex
    commit SHA) rather than a specific commit.
    """
    uses = _mutation_job(load_workflow(MUTATION_TESTING_WORKFLOW_PATH)).get("uses")
    assert isinstance(uses, str), f"jobs.mutation.uses must be a string, got {uses!r}"
    path, _, ref = uses.partition("@")
    assert path == EXPECTED_USES_PATH, (
        f"jobs.mutation.uses must reference mutation-cargo.yml, got {path!r}"
    )
    assert len(ref) == 40, (
        f"jobs.mutation.uses must pin a full 40-character commit SHA, "
        f"not a branch or tag: {ref!r}"
    )
    assert all(c in "0123456789abcdef" for c in ref), (
        f"jobs.mutation.uses must pin a lowercase hex commit SHA, "
        f"not a branch or tag: {ref!r}"
    )


def test_job_permissions_are_exactly_least_privilege() -> None:
    """The job grants contents: read and id-token: write, nothing broader."""
    permissions = _mutation_job(load_workflow(MUTATION_TESTING_WORKFLOW_PATH)).get(
        "permissions"
    )
    assert permissions == {"contents": "read", "id-token": "write"}, (
        "jobs.mutation.permissions must be exactly "
        f"{{'contents': 'read', 'id-token': 'write'}}, got {permissions!r}"
    )


def test_workflow_default_permissions_are_empty() -> None:
    """The workflow-level default token scope is empty."""
    workflow = load_workflow(MUTATION_TESTING_WORKFLOW_PATH)
    assert workflow.get("permissions") == {}, (
        f"top-level permissions must be an empty mapping, got "
        f"{workflow.get('permissions')!r}"
    )


def test_concurrency_serializes_per_ref_without_cancelling() -> None:
    """Runs queue per ref instead of cancelling one another."""
    concurrency = require_mapping(
        load_workflow(MUTATION_TESTING_WORKFLOW_PATH).get("concurrency"),
        "the workflow concurrency mapping",
    )
    assert concurrency.get("group") == "mutation-testing-${{ github.ref }}", (
        f"concurrency.group must key on the triggering ref, got "
        f"{concurrency.get('group')!r}"
    )
    assert concurrency.get("cancel-in-progress") is False, (
        f"concurrency.cancel-in-progress must be false, got "
        f"{concurrency.get('cancel-in-progress')!r}"
    )


def test_triggers_keep_schedule_and_plain_dispatch() -> None:
    """The daily schedule stays; dispatch has no legacy branch input."""
    triggers = _triggers(load_workflow(MUTATION_TESTING_WORKFLOW_PATH))
    schedule = triggers.get("schedule")
    assert schedule == [{"cron": "5 3 * * *"}], (
        f"on.schedule must be the daily 03:05 UTC cron, got {schedule!r}"
    )
    assert "workflow_dispatch" in triggers, "on.workflow_dispatch is missing"
    # A bodiless ``workflow_dispatch:`` parses as ``None``, which is a valid
    # trigger declaring no inputs; only a mapping can carry an input table.
    inputs: dict[str, object] = {}
    match triggers.get("workflow_dispatch"):
        case {"inputs": dict() as declared}:
            inputs = declared
    assert "branch" not in inputs, (
        "on.workflow_dispatch must not declare a branch input; the Actions "
        "run-workflow control selects the ref"
    )


def test_with_block_carries_the_caller_configuration() -> None:
    """The caller passes exactly the Kani excludes and feature args."""
    with_block = require_mapping(
        _mutation_job(load_workflow(MUTATION_TESTING_WORKFLOW_PATH)).get("with"),
        "jobs.mutation.with",
    )
    assert with_block == EXPECTED_WITH, (
        "jobs.mutation.with must be exactly the documented configuration "
        f"(the #[cfg(kani)] module excludes and --all-features to match "
        f"the CI baseline); expected "
        f"{EXPECTED_WITH!r}, got {with_block!r}"
    )
