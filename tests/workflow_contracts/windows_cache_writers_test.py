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

_PROFILE_PREDICATE = re.compile(r"inputs\.profile\s*(==|!=)\s*'([a-z]+)'")
_KEY_OUTPUT = re.compile(r"steps\.keys\.outputs\.([a-z-]+)")


def save_runs_for_profile(condition: str, profile: str) -> bool:
    """Return whether a save step's `if:` admits `profile`.

    Only the profile clauses are evaluated. The trunk-push and cache-hit
    clauses are identical across families, so they cannot give one family two
    writers and another none.

    The expression is read as a disjunction of conjunctions: `||` separates
    alternatives, and every profile predicate within an alternative must hold.
    That covers `== 'lint'`, `!= 'smoke'`, and
    `== 'lint' || inputs.profile == 'gate'`, the last of which admits both
    profiles and is exactly how a second writer would arrive.

    Parentheses are rejected rather than guessed at, because a reader that
    silently misgroups them would report the ownership this file exists to
    check while the real condition said something else.

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
    assert "(" not in condition, (
        "the profile reader cannot group a parenthesised condition; extend it "
        f"rather than letting it guess: {condition!r}"
    )
    for alternative in condition.split("||"):
        predicates = _PROFILE_PREDICATE.findall(alternative)
        if all(
            (value == profile) if operator == "==" else (value != profile)
            for operator, value in predicates
        ):
            return True
    return False


def windows_save_profiles() -> dict[str, list[str]]:
    """Return every save profile each Windows job passes, keyed by job name.

    A list rather than one value: nothing stops a job calling the action twice
    with different profiles, and that is one way to give a key two writers.
    Keeping only the last call would hide it.

    Returns
    -------
    dict[str, list[str]]
        Job name to the profiles of its `mode: save` calls, in declaration
        order.
    """
    workflow = load_workflow(CI_WINDOWS_WORKFLOW_PATH)
    jobs = require_mapping(workflow.get("jobs"), "ci-windows.yml jobs")
    profiles: dict[str, list[str]] = {}
    for job_name in jobs:
        for step in job_steps(workflow, str(job_name)):
            if str(step.get("uses", "")) != WINDOWS_CACHE_ACTION:
                continue
            with_ = require_mapping(step.get("with"), f"{job_name} cache call")
            if with_.get("mode") == "save":
                profiles.setdefault(str(job_name), []).append(str(with_.get("profile")))
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
            for job_name, job_profiles in profiles.items()
            if any(
                save_runs_for_profile(condition, profile) for profile in job_profiles
            )
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


def test_a_second_save_call_in_one_job_is_counted() -> None:
    """A job calling the action twice must not hide its first profile.

    Scenario: `windows_save_profiles` keys by job name, so keeping only the
    last `mode: save` call would let a job add a second call with the other
    profile and gain a key without the ownership check noticing. Invariant:
    every save call's profile is retained, so both are evaluated.
    """
    profiles = {"lint-windows": ["lint", "gate"]}
    condition = "inputs.mode == 'save' && inputs.profile == 'gate'"
    admitted = [
        job_name
        for job_name, job_profiles in profiles.items()
        if any(save_runs_for_profile(condition, profile) for profile in job_profiles)
    ]
    assert admitted == ["lint-windows"], (
        "a job whose second save call passes the other profile must be counted "
        f"as a writer for that profile's keys, got {admitted!r}"
    )


def test_a_parenthesised_condition_is_refused_rather_than_guessed() -> None:
    """An unreadable condition must fail loudly, not be misread as narrow.

    Scenario: the reader groups `||` and `&&` positionally, which parentheses
    would invalidate. Invariant: a parenthesised condition raises rather than
    returning a confident answer, so extending the action forces extending the
    reader instead of silently weakening the ownership check.
    """
    with pytest.raises(AssertionError, match="parenthesised"):
        save_runs_for_profile(
            "(inputs.profile == 'lint' || inputs.profile == 'gate')", "gate"
        )


@pytest.mark.parametrize(
    ("condition", "profile", "expected"),
    [
        ("inputs.profile == 'lint'", "lint", True),
        ("inputs.profile == 'lint'", "gate", False),
        ("inputs.profile != 'smoke'", "lint", True),
        ("inputs.profile != 'smoke'", "gate", True),
        ("inputs.mode == 'save'", "gate", True),
        ("inputs.profile == 'lint' || inputs.profile == 'gate'", "lint", True),
        ("inputs.profile == 'lint' || inputs.profile == 'gate'", "gate", True),
        ("inputs.profile == 'lint' || inputs.profile == 'smoke'", "gate", False),
        (
            (
                "inputs.mode == 'save' && inputs.profile == 'lint' && "
                "steps.keys.outputs.writer == 'true'"
            ),
            "gate",
            False,
        ),
    ],
    ids=[
        "equality-admits-its-own-profile",
        "equality-excludes-another",
        "inequality-admits-lint",
        "inequality-admits-gate",
        "no-profile-clause-admits-any",
        "disjunction-admits-its-first-alternative",
        "disjunction-admits-its-second-alternative",
        "disjunction-excludes-a-profile-in-neither-alternative",
        "conjunction-with-other-clauses-still-narrows",
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
