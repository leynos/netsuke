"""Hold the one-writer-per-key rule across the two Windows jobs.

Splitting the Windows gate into `lint-windows` and `build-test-windows` split
which job fills which cache family, so ownership had to be split with it: the
lint job saves `tools` and `whitaker`, the paths it installs into, and the test
job saves `registry` and `sccache`, the two its compile fills. Both restore all
four.

Two savers for one key is the failure this guards. GitHub Actions cache entries
are immutable, so a second saver does not overwrite the first; it races for the
reservation and loses, and warm behaviour then depends on which job finished
first. Nothing in the workflow makes that visible, and nothing in the action
makes it impossible, because the profile gate lives in a `if:` expression that
an edit can widen by one character.

So this reads the action's save conditions rather than a list of job names, and
evaluates each one against each profile the Windows jobs actually pass. A save
step widened from `== 'lint'` to `!= 'smoke'` gains a second writer and fails
here.

Run via ``make test-workflow-contracts``.
"""

import re

import pytest
import yaml
from workflow_loading import (
    CI_WINDOWS_WORKFLOW_PATH,
    REPO_ROOT,
    job_steps,
    load_workflow,
    require_mapping,
)

WINDOWS_CACHE_ACTION = "./.github/actions/windows-gate-cache"
ACTION_PATH = REPO_ROOT / ".github" / "actions" / "windows-gate-cache" / "action.yml"

#: The key families the Windows action publishes, and the job that must be the
#: sole writer of each. Stated here so a family losing its writer entirely is a
#: failure too, not only a family gaining a second one.
EXPECTED_KEY_WRITERS = {
    "registry": "build-test-windows",
    "sccache": "build-test-windows",
    "tools": "lint-windows",
    "whitaker": "lint-windows",
}

_PROFILE_EQ = re.compile(r"inputs\.profile\s*==\s*'([a-z]+)'")
_PROFILE_NE = re.compile(r"inputs\.profile\s*!=\s*'([a-z]+)'")
_KEY_OUTPUT = re.compile(r"steps\.keys\.outputs\.([a-z-]+)")


def save_runs_for_profile(condition: str, profile: str) -> bool:
    """Return whether a save step's `if:` admits `profile`.

    Only the profile clause is evaluated. The trunk-push and cache-hit clauses
    are the same for every family, so they cannot make one family have two
    writers and another none.

    Parameters
    ----------
    condition:
        The step's `if:` expression.
    profile:
        The profile a caller passes to the action.

    Returns
    -------
    bool
        True when a call with `profile` would run this save step.
    """
    if match := _PROFILE_EQ.search(condition):
        return match.group(1) == profile
    if match := _PROFILE_NE.search(condition):
        return match.group(1) != profile
    return True


def windows_save_profiles() -> dict[str, str]:
    """Return each Windows job's save profile, keyed by job name.

    Returns
    -------
    dict[str, str]
        Job name to the `profile` its `mode: save` call passes.
    """
    workflow = load_workflow(CI_WINDOWS_WORKFLOW_PATH)
    jobs = require_mapping(workflow.get("jobs"), "ci-windows.yml jobs")
    profiles: dict[str, str] = {}
    for job_name in jobs:
        for step in job_steps(workflow, str(job_name)):
            if str(step.get("uses", "")) != WINDOWS_CACHE_ACTION:
                continue
            with_ = require_mapping(step.get("with"), f"{job_name} cache call")
            if with_.get("mode") == "save":
                profiles[str(job_name)] = str(with_.get("profile"))
    return profiles


def action_save_steps() -> list[tuple[str, str]]:
    """Return each save step of the action as a key family and condition.

    Returns
    -------
    list[tuple[str, str]]
        The key family the step publishes and its `if:` expression.
    """
    action = yaml.safe_load(ACTION_PATH.read_text(encoding="utf-8"))
    runs = require_mapping(action.get("runs"), "the action's runs block")
    steps: list[tuple[str, str]] = []
    declared = runs.get("steps")
    assert isinstance(declared, list), (
        f"the action's runs block must declare a list of steps, got {declared!r}"
    )
    for entry in declared:
        step = require_mapping(entry, "an action step")
        name = str(step.get("name", ""))
        if not name.startswith("Save "):
            continue
        with_ = require_mapping(step.get("with"), f"{name}'s with block")
        family = _KEY_OUTPUT.search(str(with_.get("key", "")))
        assert family, f"{name} must publish a rendered key"
        steps.append((family.group(1), str(step.get("if", ""))))
    return steps


def test_every_windows_cache_key_has_exactly_one_writer() -> None:
    """No Windows cache key may be saved by both jobs, or by neither.

    Scenario: the gate is two concurrent jobs, so the four key families are
    split between them by the action's `profile` input. Invariant: evaluating
    every save condition against every profile the two jobs pass yields exactly
    one writing job per family, and it is the job that fills that family's
    paths. Widening one save condition so both profiles match it, which is a
    one-token edit, gains a second writer and fails here.
    """
    profiles = windows_save_profiles()
    assert set(profiles) == set(EXPECTED_KEY_WRITERS.values()), (
        "both Windows jobs must publish through the cache action, "
        f"got save profiles {profiles!r}"
    )

    writers: dict[str, list[str]] = {family: [] for family in EXPECTED_KEY_WRITERS}
    for family, condition in action_save_steps():
        assert family in writers, (
            f"the action saves an unrecognised key family {family!r}; add it to "
            "EXPECTED_KEY_WRITERS with its owner"
        )
        writers[family] = [
            job_name
            for job_name, profile in profiles.items()
            if save_runs_for_profile(condition, profile)
        ]

    duplicated = {f: jobs for f, jobs in writers.items() if len(jobs) > 1}
    assert not duplicated, (
        "each Windows cache key must have exactly one writer; Actions cache "
        "entries are immutable, so a second saver races for the reservation "
        f"and warm behaviour depends on which job finished first. Got {duplicated!r}"
    )

    actual = {family: jobs[0] if jobs else None for family, jobs in writers.items()}
    assert actual == EXPECTED_KEY_WRITERS, (
        "each Windows cache key must be written by the job that fills its "
        f"paths; expected {EXPECTED_KEY_WRITERS!r}, got {actual!r}"
    )


@pytest.mark.parametrize(
    ("condition", "profile", "expected"),
    [
        ("inputs.profile == 'lint'", "lint", True),
        ("inputs.profile == 'lint'", "gate", False),
        ("inputs.profile != 'smoke'", "lint", True),
        ("inputs.profile != 'smoke'", "gate", True),
        ("inputs.mode == 'save'", "gate", True),
    ],
    ids=[
        "equality-admits-its-own-profile",
        "equality-excludes-another",
        "inequality-admits-lint",
        "inequality-admits-gate",
        "no-profile-clause-admits-any",
    ],
)
def test_save_condition_evaluation(
    condition: str, profile: str, *, expected: bool
) -> None:
    """The condition reader must admit exactly the profiles the runner would.

    Scenario: the ownership check is only as good as its reading of the `if:`
    expression. Invariant: equality admits one profile, inequality admits every
    other, and a condition naming no profile admits all. Getting the
    inequality case wrong is what would let a widened save step pass unnoticed.
    """
    assert save_runs_for_profile(condition, profile) is expected, (
        f"the profile clause of {condition!r} should "
        f"{'admit' if expected else 'exclude'} profile {profile!r}"
    )
