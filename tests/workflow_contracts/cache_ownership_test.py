"""Hold cache ownership and provider contracts.

Ubicloud destroys the runner VM after every job, so warm state arrives only
through archives. That makes ownership the whole design: each mutable path has
exactly one cache step, one action at one pin serves every lane, and Cargo's
build tree is archived nowhere at all because sccache owns compiler output.
These checks pin that arrangement so a workflow edit cannot quietly add a
second owner.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
from cache_contract_data import (
    ACTION_DIR,
    CACHE_ACTION_CALLERS,
    CACHE_RESTORE,
    CACHE_SAVE,
    EXTERNAL_CACHE_PROVIDER,
    FORBIDDEN_CACHE_PATHS,
    GITHUB_CACHE_SOURCES,
    OBSERVATION_SOURCES,
    SCCACHE_CREDENTIALS_ACTION,
    SETUP_RUST_ACTION,
    TRUNK_TRIGGERED_WORKFLOWS,
    UBICLOUD_CACHE_SOURCES,
    WORKFLOW_DIR,
    cache_steps,
    declared_paths,
    lane_steps,
)
from runner_placement_invariants import (
    UBICLOUD_LABELS,
    has_single_cache_owner,
)
from workflow_loading import (
    job_steps,
    load_workflow,
    named_step,
    require_mapping,
)

if typ.TYPE_CHECKING:
    from pathlib import Path


@pytest.mark.parametrize(
    ("source", "job_name"), UBICLOUD_CACHE_SOURCES + GITHUB_CACHE_SOURCES
)
def test_every_lane_uses_the_single_pinned_cache_action(
    source: Path, job_name: str | None
) -> None:
    """Require one cache action at one pin across every lane.

    Ubicloud's transparent cache intercepts `actions/cache` at this version,
    verified from the Ubicloud console listing on 2026-09-03, so the
    deprecated `ubicloud/cache` fork buys nothing and a second action would be
    a second thing to audit.
    """
    for step in cache_steps(lane_steps(source, job_name)):
        uses = str(step.get("uses"))
        assert uses in {CACHE_RESTORE, CACHE_SAVE}, (
            f"{source.name} step {step.get('name')!r} must use the pinned "
            f"cache action, got {uses!r}"
        )


@pytest.mark.parametrize(
    ("source", "job_name"), UBICLOUD_CACHE_SOURCES + GITHUB_CACHE_SOURCES
)
def test_no_lane_archives_a_cargo_build_tree(
    source: Path, job_name: str | None
) -> None:
    """Reject any cache step that claims Cargo's `target` directory.

    sccache owns compiler output for every build shape, so a `target` archive
    would be a second owner of the same bytes, invalidated far more often than
    it helped. This holds on Windows as well as on Ubicloud. Kani's
    directories are a tool payload rather than a build tree.
    """
    for step in cache_steps(lane_steps(source, job_name)):
        offenders = [
            path for path in declared_paths(step) if path in FORBIDDEN_CACHE_PATHS
        ]
        assert not offenders, (
            f"{source.name} step {step.get('name')!r} archives a build tree: "
            f"{offenders!r}"
        )


def test_each_job_calls_only_its_own_lane_cache_action() -> None:
    """Keep each job to the cache action for its platform, and no other."""
    for (workflow_name, job_name), action in CACHE_ACTION_CALLERS.items():
        steps = job_steps(load_workflow(WORKFLOW_DIR / workflow_name), job_name)
        references = {
            str(step.get("uses"))
            for step in steps
            if str(step.get("uses", "")).startswith("./.github/actions/")
            and str(step.get("uses")) != SCCACHE_CREDENTIALS_ACTION
        }
        assert references == {action}, (
            f"{workflow_name} {job_name} must call {action} and no other cache "
            f"action, got {references!r}"
        )


@pytest.mark.parametrize(
    ("source", "job_name"), UBICLOUD_CACHE_SOURCES + GITHUB_CACHE_SOURCES
)
def test_each_cached_path_has_exactly_one_owner(
    source: Path, job_name: str | None
) -> None:
    """Reject a second cache step claiming a path another already owns."""
    for mode, matcher in (("restore", "/restore@"), ("save", "/save@")):
        owners = [
            (str(step.get("name")), path)
            for step in cache_steps(lane_steps(source, job_name))
            if matcher in str(step.get("uses", ""))
            for path in declared_paths(step)
        ]
        assert has_single_cache_owner(owners), (
            f"{source.name} {job_name or 'composite'} {mode} steps claim a "
            f"path twice: {owners!r}"
        )


@pytest.mark.parametrize(
    ("source", "job_name"), UBICLOUD_CACHE_SOURCES + GITHUB_CACHE_SOURCES
)
def test_restores_precede_every_install(source: Path, job_name: str | None) -> None:
    """Require the caches to be read before any tool or toolchain install.

    A restore that lands after an installer pays for work the archive already
    holds, which is the failure the cache exists to remove.
    """
    steps = lane_steps(source, job_name)
    restore_indices = [
        index
        for index, step in enumerate(steps)
        if "/restore@" in str(step.get("uses", ""))
    ]
    if not restore_indices:
        pytest.skip(f"{source.name} {job_name} declares no restore step")
    install_indices = [
        index
        for index, step in enumerate(steps)
        if str(step.get("name", "")).startswith("Install ")
        or SETUP_RUST_ACTION in str(step.get("uses", ""))
    ]
    if install_indices:
        assert max(restore_indices) < min(install_indices), (
            f"{source.name} {job_name} must restore before its first install"
        )


@pytest.mark.parametrize(("source", "job_name"), OBSERVATION_SOURCES)
def test_every_cache_bearing_job_records_its_observations(
    source: Path, job_name: str | None
) -> None:
    """Require the rendered key and hit result on every run, hit or miss.

    A summary that runs only on success hides the cold case the cache exists
    to eliminate, so the step also carries `if: always()`.
    """
    steps = lane_steps(source, job_name)
    observation = next(
        step
        for step in steps
        if str(step.get("name", "")).startswith("Record cache observation")
    )
    assert str(observation.get("if", "")).startswith("always()"), (
        f"{source.name} {job_name} must record cache observations on every run"
    )
    script = str(observation.get("run", ""))
    assert "GITHUB_STEP_SUMMARY" in script, (
        f"{source.name} {job_name} must publish its observations to the summary"
    )
    assert "key" in script or "prefix" in script, (
        f"{source.name} {job_name} must record the rendered cache key"
    )


@pytest.mark.parametrize("workflow_name", TRUNK_TRIGGERED_WORKFLOWS)
def test_cache_writers_run_on_a_push_to_the_trunk(workflow_name: str) -> None:
    """Require a trunk trigger, without which no generation is ever written."""
    workflow = load_workflow(WORKFLOW_DIR / workflow_name)
    triggers = require_mapping(workflow.get("on"), f"{workflow_name} triggers")
    push = require_mapping(triggers.get("push"), f"{workflow_name} push trigger")
    assert push.get("branches") == ["main"], (
        f"{workflow_name} must run on a push to main, got {push!r}"
    )


def test_shared_actions_delegate_cache_ownership_to_the_caller() -> None:
    """Require every shared action to leave cache ownership to the workflow.

    `setup-rust` caches `target/${BUILD_PROFILE}` and `generate-coverage`
    caches the whole `target` tree whenever their `cache-provider` is
    `github`, so passing `external` is what keeps a build tree out of every
    lane this repository owns.
    """
    jobs = (
        ("ci.yml", "build-test"),
        ("ci.yml", "kani-smoke"),
        ("ci-windows.yml", "build-test-windows"),
        ("ci-windows.yml", "windows-native-recipe-smoke"),
        ("coverage-main.yml", "coverage-upload"),
        ("netsukefile-test.yml", "netsukefile"),
        ("release.yml", "windows-native-recipe-smoke"),
    )
    for workflow_name, job_name in jobs:
        workflow = load_workflow(WORKFLOW_DIR / workflow_name)
        setup = named_step(job_steps(workflow, job_name), "Setup Rust")
        assert str(setup.get("uses", "")).startswith(SETUP_RUST_ACTION), (
            f"{workflow_name} {job_name} must use shared setup-rust"
        )
        inputs = require_mapping(setup.get("with"), "Setup Rust inputs")
        assert inputs.get("cache-provider") == EXTERNAL_CACHE_PROVIDER, (
            f"{workflow_name} {job_name} must disable setup-rust's GitHub cache"
        )
        assert inputs.get("use-sccache") == "false", (
            f"{workflow_name} {job_name} must not enable a second sccache owner"
        )


def test_coverage_and_whitaker_actions_delegate_their_archives() -> None:
    """Require the coverage and Whitaker actions to own no cache of their own."""
    for workflow_name, job_name, step_name in (
        ("ci.yml", "build-test", "Test and Measure Coverage"),
        ("coverage-main.yml", "coverage-upload", "Test and Measure Coverage"),
        ("ci.yml", "build-test", "Install Whitaker"),
        ("ci-windows.yml", "build-test-windows", "Install Whitaker"),
    ):
        workflow = load_workflow(WORKFLOW_DIR / workflow_name)
        step = named_step(job_steps(workflow, job_name), step_name)
        inputs = require_mapping(step.get("with"), f"{step_name} inputs")
        assert inputs.get("cache-provider") == EXTERNAL_CACHE_PROVIDER, (
            f"{workflow_name} {job_name} {step_name} must delegate its cache"
        )


def test_workflows_do_not_reintroduce_source_tool_builds_or_stale_providers() -> None:
    """Reject source tool builds and the retired Namespace cache action."""
    workflow_text = "\n".join(
        path.read_text(encoding="utf-8")
        for path in sorted(WORKFLOW_DIR.glob("*.yml"))
        + sorted(ACTION_DIR.glob("*/action.yml"))
    )
    assert "cargo install " not in workflow_text, (
        "CI tools must use trusted prebuilt binaries rather than source builds"
    )
    assert "nscloud-cache-action" not in workflow_text, (
        "the Namespace cache volume action must not return"
    )
    assert "ubicloud/cache" not in workflow_text, (
        "the deprecated ubicloud/cache fork must not return"
    )
    assert "namespace-profile-" not in workflow_text, (
        "no workflow may name a Namespace runner profile"
    )
    for label in UBICLOUD_LABELS:
        assert label in workflow_text, f"{label} must be used by some workflow"
