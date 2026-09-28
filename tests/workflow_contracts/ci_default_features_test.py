"""Contract tests for the CI workflow's default-feature lane.

Release binaries build Netsuke's default feature set, which leaves out the
``lint`` feature (``netsuke check``) until v0.2.0; every other merge-gate lane
builds ``--all-features``. The ``default-features`` job in
``ci-default-features.yml`` is therefore the only evidence that the release
feature set compiles, stays warning-free, and passes its tests. Each check
below pins one way that evidence could quietly disappear: the lane stops being
called, stops running a gate, or runs a gate that turns every feature back on.

Run via ``make test-workflow-contracts``.
"""

import re

import pytest
from makefile_recipes import load_makefile, makefile_recipe
from makefile_variables import expand_makefile_variables, makefile_definitions
from workflow_loading import (
    CI_DEFAULT_FEATURES_WORKFLOW_PATH,
    CI_WORKFLOW_PATH,
    job_steps,
    load_workflow,
    named_step,
    require_mapping,
    workflow_job,
)

LANE_JOB = "default-features"
LANE_TARGETS = ("lint-default-features", "test-default-features")
#: Any Cargo argument that selects features beyond the default set.
FEATURE_SELECTION = re.compile(
    r"--all-features|--features\b|-F\b|--no-default-features"
)


def lane_recipe(target: str) -> str:
    """Return ``target``'s recipe with the Makefile's variables expanded."""
    makefile = load_makefile()
    return expand_makefile_variables(
        makefile_recipe(target), makefile_definitions(makefile)
    )


def test_ci_calls_the_default_feature_lane_with_the_workflow_pin() -> None:
    """``ci.yml`` must call the lane and pass it the gate's nextest pin.

    GitHub does not expose the ``env`` context to a reusable workflow's
    ``with`` block, so the caller repeats the literal; this holds the copies
    equal, as the Windows gate's contract does for its own call.
    """
    caller = load_workflow(CI_WORKFLOW_PATH)
    caller_env = require_mapping(caller.get("env"), "the CI workflow env")
    call = workflow_job(caller, LANE_JOB)
    uses = call.get("uses")
    assert uses == "./.github/workflows/ci-default-features.yml", (
        f"the default-feature lane must be a local reusable workflow, got {uses!r}"
    )
    inputs = require_mapping(call.get("with"), "the default-feature call inputs")
    assert inputs.get("nextest-version") == caller_env.get("NEXTEST_VERSION"), (
        "the default-feature call must pass the workflow's NEXTEST_VERSION pin"
    )
    called = load_workflow(CI_DEFAULT_FEATURES_WORKFLOW_PATH)
    called_env = require_mapping(called.get("env"), "the default-feature workflow env")
    assert called_env.get("NEXTEST_VERSION") == "${{ inputs['nextest-version'] }}", (
        "the default-feature workflow must read NEXTEST_VERSION from its input"
    )


@pytest.mark.parametrize(
    ("step_name", "target"),
    [
        ("Lint with the default features", "lint-default-features"),
        ("Test with the default features", "test-default-features"),
    ],
)
def test_the_lane_runs_each_default_feature_gate(step_name: str, target: str) -> None:
    """The lane must run both Makefile gates, each in its own named step."""
    steps = job_steps(load_workflow(CI_DEFAULT_FEATURES_WORKFLOW_PATH), LANE_JOB)
    run = named_step(steps, step_name).get("run")
    assert run == f"make {target}", (
        f"{step_name!r} must run `make {target}`, got {run!r}"
    )


def test_the_lane_requires_ninja() -> None:
    """Ninja-dependent tests skip silently unless the lane demands Ninja."""
    job = workflow_job(load_workflow(CI_DEFAULT_FEATURES_WORKFLOW_PATH), LANE_JOB)
    env = require_mapping(job.get("env"), "the default-feature job env")
    assert env.get("NETSUKE_REQUIRE_NINJA") == "1", (
        "the default-feature lane must set NETSUKE_REQUIRE_NINJA so Ninja tests run"
    )


@pytest.mark.parametrize("target", LANE_TARGETS)
def test_default_feature_recipes_select_no_extra_features(target: str) -> None:
    """Neither recipe may turn on a feature release binaries do not build."""
    recipe = lane_recipe(target)
    assert not FEATURE_SELECTION.search(recipe), (
        f"`make {target}` must build only the default feature set:\n{recipe}"
    )
    assert "-D warnings" in recipe, f"`make {target}` must deny warnings"


def test_default_feature_recipes_cover_the_whole_workspace() -> None:
    """The lane must lint and test every target, doctests included."""
    lint = lane_recipe("lint-default-features")
    assert "cargo doc --workspace" in lint, "the lint lane must build rustdoc"
    assert "clippy --workspace --all-targets" in lint, "Clippy must cover every target"
    test = lane_recipe("test-default-features")
    assert "nextest run --workspace --all-targets" in test, (
        "nextest must cover every target"
    )
    assert "test --workspace --doc" in test, "the lane must run the doctests"
