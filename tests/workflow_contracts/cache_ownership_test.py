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
    DELEGATING_ACTION_STEPS,
    EXTERNAL_CACHE_PROVIDER,
    FORBIDDEN_SOURCE_BUILDS,
    GITHUB_CACHE_SOURCES,
    NON_CACHE_ACTIONS,
    OBSERVATION_SOURCES,
    RUST_BUILD_RELEASE_ACTION,
    RUST_BUILD_RELEASE_PIN_VARIABLE,
    RUST_BUILD_RELEASE_PIN_WORKFLOW,
    SETUP_RUST_ACTION,
    SETUP_RUST_DELEGATING_JOBS,
    SOURCE_BUILD_EXCEPTIONS,
    TARGET_ARCHIVE_OWNERS,
    TRUNK_TRIGGERED_WORKFLOWS,
    UBICLOUD_CACHE_SOURCES,
    WORKFLOW_DIR,
    cache_steps,
    declared_paths,
    is_build_tree,
    is_source_built,
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


def _require_cache_steps(source: Path, job_name: str | None) -> list[dict[str, object]]:
    """Return a lane's cache steps, failing when it declares none.

    Every contract below quantifies over those steps, so a lane that lost them
    entirely would satisfy all of them vacuously. That is the failure mode
    these contracts exist to catch, so absence is an error rather than a pass.

    Parameters
    ----------
    source
        Workflow or composite action file to read.
    job_name
        Job to inspect, or ``None`` for a composite action.

    Returns
    -------
    list[dict[str, object]]
        The lane's cache steps, in declaration order.
    """
    steps = cache_steps(lane_steps(source, job_name))
    assert steps, (
        f"{source.name} {job_name or 'composite'} declares no cache steps, so "
        "every ownership contract would pass over it in silence"
    )
    return steps


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
    for step in _require_cache_steps(source, job_name):
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
    for step in _require_cache_steps(source, job_name):
        offenders = [path for path in declared_paths(step) if is_build_tree(path)]
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
            and str(step.get("uses")) not in NON_CACHE_ACTIONS
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
            for step in _require_cache_steps(source, job_name)
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
    assert restore_indices, (
        f"{source.name} {job_name or 'composite'} must declare a restore step; "
        "skipping the absent case would let deleting the only restore satisfy "
        "this ordering contract"
    )
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
    env = require_mapping(observation.get("env", {}), "observation env")
    referenced = " ".join([script, *(str(value) for value in env.values())])
    assert "steps.keys.outputs" in referenced or "CACHE_KEY" in referenced, (
        f"{source.name} {job_name} must record the rendered key itself, not "
        "merely the word 'key'"
    )
    assert "cache-hit" in referenced or "cache-matched-key" in referenced, (
        f"{source.name} {job_name} must record the restore result beside its key"
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


def test_no_shared_action_enables_its_own_target_archive() -> None:
    """Require every shared action's built-in cache to stay caller-owned.

    `setup-rust` archives `target/${BUILD_PROFILE}` and `generate-coverage`
    the whole tree when their `cache-provider` is `github`, so a caller that
    forgets `external` reintroduces the build-tree archive this repository
    removed. `rust-build-release` forwards the same input to its nested
    `setup-rust`, which is what keeps the packaging lane clean.
    """
    for workflow_name, job_name, step_name in TARGET_ARCHIVE_OWNERS:
        workflow = load_workflow(WORKFLOW_DIR / workflow_name)
        step = named_step(job_steps(workflow, job_name), step_name)
        inputs = require_mapping(step.get("with"), f"{step_name} inputs")
        assert inputs.get("cache-provider") == EXTERNAL_CACHE_PROVIDER, (
            f"{workflow_name} {job_name} {step_name} must not enable its own "
            "target archive"
        )


@pytest.mark.parametrize(("workflow_name", "job_name"), SETUP_RUST_DELEGATING_JOBS)
def test_shared_actions_delegate_cache_ownership_to_the_caller(
    workflow_name: str, job_name: str
) -> None:
    """Require every shared action to leave cache ownership to the workflow.

    `setup-rust` caches `target/${BUILD_PROFILE}` and `generate-coverage`
    caches the whole `target` tree whenever their `cache-provider` is
    `github`, so passing `external` is what keeps a build tree out of every
    lane this repository owns.
    """
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


@pytest.mark.parametrize(
    ("workflow_name", "job_name", "step_name"), DELEGATING_ACTION_STEPS
)
def test_coverage_and_whitaker_actions_delegate_their_archives(
    workflow_name: str, job_name: str, step_name: str
) -> None:
    """Require the coverage and Whitaker actions to own no cache of their own."""
    workflow = load_workflow(WORKFLOW_DIR / workflow_name)
    step = named_step(job_steps(workflow, job_name), step_name)
    inputs = require_mapping(step.get("with"), f"{step_name} inputs")
    assert inputs.get("cache-provider") == EXTERNAL_CACHE_PROVIDER, (
        f"{workflow_name} {job_name} {step_name} must delegate its cache"
    )


def test_the_orthohelp_key_carries_the_binstall_provisioning_pin() -> None:
    """Hold the `cargo-orthohelp` key's action pin equal to the `uses:` pin.

    The entry owns `~/.cargo/bin` as a directory, which is this repository's
    rule for tool directories. That directory also holds the `cargo-binstall`
    the pinned `rust-build-release` action provisions, so an entry keyed on the
    tool version alone would not turn over when the action's binstall version
    moved, and a restore would put the older binary back over the one the
    action had just installed. Restating the revision in an environment
    variable is what lets the key carry it, and restating it is exactly what
    can drift, so the two are held equal here.
    """
    path = WORKFLOW_DIR / RUST_BUILD_RELEASE_PIN_WORKFLOW
    workflow = load_workflow(path)
    job = require_mapping(
        require_mapping(workflow.get("jobs"), "jobs").get("build"), "build job"
    )
    env = require_mapping(job.get("env"), "build job env")
    declared = str(env.get(RUST_BUILD_RELEASE_PIN_VARIABLE, ""))
    assert declared, (
        f"{RUST_BUILD_RELEASE_PIN_WORKFLOW} must declare "
        f"{RUST_BUILD_RELEASE_PIN_VARIABLE} so the orthohelp key can carry it"
    )
    references = [
        str(step.get("uses"))
        for step in job_steps(workflow, "build")
        if str(step.get("uses", "")).startswith(RUST_BUILD_RELEASE_ACTION)
    ]
    assert references, (
        f"{RUST_BUILD_RELEASE_PIN_WORKFLOW} must call {RUST_BUILD_RELEASE_ACTION}"
    )
    pins = {reference.rsplit("@", 1)[1] for reference in references}
    assert pins == {declared}, (
        f"{RUST_BUILD_RELEASE_PIN_VARIABLE} is {declared!r} but the action is "
        f"pinned at {pins!r}; the orthohelp cache key would carry a revision "
        "the lane does not run"
    )
    keys = [
        str(require_mapping(step.get("with"), "cache step inputs").get("key", ""))
        for step in cache_steps(job_steps(workflow, "build"))
    ]
    orthohelp_keys = [key for key in keys if "netsuke-orthohelp-" in key]
    assert orthohelp_keys, "the packaging lane must declare an orthohelp cache key"
    for key in orthohelp_keys:
        assert RUST_BUILD_RELEASE_PIN_VARIABLE in key, (
            f"the orthohelp key must carry {RUST_BUILD_RELEASE_PIN_VARIABLE}, "
            f"got {key!r}"
        )


def test_workflows_do_not_reintroduce_source_tool_builds_or_stale_providers() -> None:
    """Reject source tool builds and the retired Namespace cache action."""
    workflow_text = "\n".join(
        path.read_text(encoding="utf-8")
        for path in sorted({*WORKFLOW_DIR.glob("*.yml"), *WORKFLOW_DIR.glob("*.yaml")})
        + sorted(ACTION_DIR.rglob("action.yml"))
    )
    # One documented exception remains, and it is counted rather than
    # pattern-matched so a second cannot hide behind it.
    permitted = {
        name: workflow_text.count(text)
        for name, text in SOURCE_BUILD_EXCEPTIONS.items()
    }
    unexpected = {name: count for name, count in permitted.items() if count != 1}
    assert not unexpected, (
        "each documented source-build exception must appear exactly once, "
        f"got {unexpected!r}"
    )
    # A retired exception has to stay retired, checked by name rather than by
    # a shrinking count: a count would be satisfied by adding the tool back as
    # another permitted entry above.
    returned = [
        tool for tool in FORBIDDEN_SOURCE_BUILDS if is_source_built(tool, workflow_text)
    ]
    assert not returned, (
        f"{returned!r} had a source-build exception that was retired; these "
        "tools publish prebuilt archives and must not be compiled in CI"
    )
    source_builds = workflow_text.count("cargo install ")
    # One extra occurrence is the mdtablefix action's own comment naming the
    # command it guards; the exceptions themselves account for the rest.
    assert source_builds <= sum(permitted.values()) + 1, (
        "CI tools must use trusted prebuilt binaries rather than source "
        f"builds; found {source_builds} `cargo install` calls against "
        f"{sum(permitted.values())} documented exceptions"
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
