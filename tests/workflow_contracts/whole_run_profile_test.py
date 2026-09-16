"""Every coverage lane runs under the profile that carries the budget.

The whole-run budget lives in one nextest profile rather than in
``default``, so it reaches a run only if that run selects it. Nothing
else in the tree would say so: ``whole_run_value_test`` reads the
budget straight out of the file and passes whether or not any lane ever
asks for it, and the ordering assertions read it the same way. A lane
that stopped selecting the profile would run uncapped while every other
assertion about the budget still held.

The shared ``generate-coverage`` action takes no profile input, so the
``NEXTEST_PROFILE`` environment variable is the only lever, and this
contract asserts the variable each lane actually resolves rather than
its presence anywhere in the file.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
from coverage_lanes import CoverageLane, coverage_lanes_of
from lane_environment import nextest_profile_of
from nextest_budgets import global_timeout
from timeout_budgets import (
    CAPPED_PROFILE,
    COVERAGE_ACTION,
    NEXTEST_CONFIG,
    NEXTEST_PROFILE_VARIABLE,
    WATCHDOG_VARIABLE,
)


@pytest.fixture(scope="module")
def coverage_lanes() -> tuple[CoverageLane, ...]:
    """Return every step invoking the coverage action.

    Returns
    -------
    tuple[CoverageLane, ...]
        One entry per coverage step.
    """
    return coverage_lanes_of()


def test_every_coverage_lane_selects_the_capped_profile(
    coverage_lanes: tuple[CoverageLane, ...],
) -> None:
    """The budget reaches a lane only through the profile it selects.

    A lane naming no profile runs under ``default``, which carries no
    whole-run budget by design, so the omission is silent: the run
    simply has no tier two and the watchdog resumes doing its job. A
    lane naming some other profile is the same fault wearing a value.

    Proved by mutation: deleting ``NEXTEST_PROFILE`` from either
    coverage job, and setting it to ``default`` in either, each fail
    this test.
    """
    assert coverage_lanes, (
        "no coverage lane was found, so this contract would pass over an "
        "empty list; the action moved or the lane reading stopped matching it"
    )
    wrong = {
        str(lane): lane.nextest_profile
        for lane in coverage_lanes
        if lane.nextest_profile != CAPPED_PROFILE
    }
    assert not wrong, (
        f"these coverage lanes do not set {NEXTEST_PROFILE_VARIABLE} to "
        f"{CAPPED_PROFILE!r}, so the whole-run budget does not apply to them "
        f"and the cargo watchdog is doing tier two's job again, as found "
        f"versus required {CAPPED_PROFILE!r}: {wrong}; None means the lane "
        f"names no profile and so runs under default, which carries no "
        f"whole-run budget"
    )


def test_the_selected_profile_is_the_one_carrying_the_budget() -> None:
    """The variable and the configuration have to name the same profile.

    The two halves of this change live in different files and nothing
    makes them agree. Renaming the profile in `.config/nextest.toml`
    while the workflows still select the old name leaves a budget
    nothing reads and lanes selecting a profile that does not exist,
    which nextest accepts by falling back to ``default``.
    """
    config_text = NEXTEST_CONFIG.read_text(encoding="utf-8")

    assert global_timeout(config_text, CAPPED_PROFILE) is not None, (
        f"the coverage lanes select [profile.{CAPPED_PROFILE}] but that "
        f"profile declares no global-timeout, so every lane runs with no "
        f"whole-run budget while the configuration appears to set one"
    )
    assert global_timeout(config_text, "default") is None, (
        "the whole-run budget is in [profile.default], where every local "
        "`make test` reads it. It belongs to the profile CI selects: a "
        "developer's host is contended in a way CI's is not, and a cap sized "
        "from CI logs would end a local run against a figure that says "
        "nothing about it"
    )


def test_a_blank_step_declaration_fails_the_lane_assertion() -> None:
    """The masking matters because it decides whether a lane passes.

    Reading the scopes correctly is not the point on its own. This is
    the shape the fault takes in a workflow: a job selects the capped
    profile and a step hands the process an empty value, so nextest
    selects nothing and the run has no whole-run budget. A reading that
    fell through to the job would report ``ci`` and pass.
    """
    documents = {
        "ci.yml": {
            "jobs": {
                "build-test": {
                    "timeout-minutes": 60,
                    "env": {
                        NEXTEST_PROFILE_VARIABLE: CAPPED_PROFILE,
                        WATCHDOG_VARIABLE: "1800",
                    },
                    "steps": [
                        {
                            "name": "Coverage",
                            "uses": COVERAGE_ACTION,
                            "env": {NEXTEST_PROFILE_VARIABLE: ""},
                        }
                    ],
                }
            },
        }
    }

    (lane,) = coverage_lanes_of(documents)

    assert lane.nextest_profile != CAPPED_PROFILE, (
        "a step handing the process an empty NEXTEST_PROFILE selects no "
        "profile, so the lane has no whole-run budget however the job above "
        "it is written; reporting the job's value would pass the lane"
    )


@pytest.mark.parametrize(
    ("document", "job", "step", "expected"),
    [
        pytest.param(
            {"env": {NEXTEST_PROFILE_VARIABLE: "workflow"}},
            {"env": {NEXTEST_PROFILE_VARIABLE: "job"}},
            {"env": {NEXTEST_PROFILE_VARIABLE: "step"}},
            "step",
            id="the-step-wins",
        ),
        pytest.param(
            {"env": {NEXTEST_PROFILE_VARIABLE: "workflow"}},
            {"env": {NEXTEST_PROFILE_VARIABLE: "job"}},
            {},
            "job",
            id="then-the-job",
        ),
        pytest.param(
            {"env": {NEXTEST_PROFILE_VARIABLE: "workflow"}},
            {},
            {},
            "workflow",
            id="then-the-workflow",
        ),
        pytest.param({}, {}, {}, None, id="nowhere-reads-as-absent"),
        pytest.param(
            {},
            {"env": {NEXTEST_PROFILE_VARIABLE: "   "}},
            {},
            "",
            id="a-blank-with-nothing-outside-it",
        ),
        pytest.param(
            {"env": {NEXTEST_PROFILE_VARIABLE: "workflow"}},
            {"env": {NEXTEST_PROFILE_VARIABLE: "ci"}},
            {"env": {NEXTEST_PROFILE_VARIABLE: ""}},
            "",
            id="a-blank-step-masks-the-job",
        ),
    ],
)
def test_the_profile_is_resolved_from_every_environment_scope(
    document: dict[str, typ.Any],
    job: dict[str, typ.Any],
    step: dict[str, typ.Any],
    expected: str | None,
) -> None:
    """Step, then job, then workflow, as GitHub resolves them.

    Both lanes here set the variable at job level, so against the real
    tree a reading that consulted only the job would agree with this
    one. The scopes it never reaches are what these cases execute.

    The blank cases are the ones that matter most. An interpolated
    expression that resolves to nothing writes an empty value, and
    GitHub takes the most specific declaration, so the step's blank is
    what the process receives and the job's profile never reaches it.
    A reading that fell through to the job would report the lane as
    selecting the capped profile while nextest selected nothing, which
    is exactly the fault this module exists to catch.
    """
    assert nextest_profile_of(document, job, step) == expected, (
        f"the profile must resolve to {expected!r} from these scopes; a "
        f"reading consulting fewer of them reports a lane as running under "
        f"default while an outer scope selected a profile"
    )
