"""Reading a job that runs the coverage action more than once.

Separated from `coverage_lane_reading_test`, whose subject is one lane
at a time, and kept apart so neither module outgrows the 400-line limit
the Python lint gate enforces.

Every workflow in this repository runs the coverage action at most once
per job, so nothing here can be driven against the real tree. A reading
that returned a job's first coverage step, or its last, would satisfy
every assertion the repository's own workflows support: one lane per job
is indistinguishable from every lane there. These documents are written
so that it does not.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
from coverage_lanes import CoverageLane, coverage_lanes_of
from timeout_budgets import WATCHDOG_VARIABLE
from timeout_ordering_test import _budgets_per_job, required_ceiling

COVERAGE_STEP: typ.Final[str] = (
    "leynos/shared-actions/.github/actions/generate-coverage@abc123"
)


def _two_step_workflow() -> dict[str, dict[str, object]]:
    """Return a workflow whose one job runs the coverage action twice.

    The second step sets its own watchdog, so the reading has to resolve
    step before job on one lane and fall back to the job on the other.

    Returns
    -------
    dict
        A document with a single `build-test` job of two coverage steps.
    """
    return {
        "jobs": {
            "build-test": {
                "timeout-minutes": 60,
                "env": {WATCHDOG_VARIABLE: 1800},
                "steps": [
                    {"name": "cover one", "uses": COVERAGE_STEP},
                    {
                        "name": "cover two",
                        "uses": COVERAGE_STEP,
                        "env": {WATCHDOG_VARIABLE: 2700},
                    },
                ],
            }
        }
    }


def test_a_job_running_the_action_twice_yields_both_lanes() -> None:
    """Every matching step is a lane, and each carries its own watchdog.

    The grouping and the ceiling arithmetic were exercised from lanes
    built by hand, so nothing said that reading a document produces two
    of them. A reading that returned the first coverage step, or the
    last, would have satisfied every assertion in this suite: this
    repository's two lanes sit in different jobs, so one lane per job is
    indistinguishable from every lane there.
    """
    lanes = coverage_lanes_of({"ci.yml": _two_step_workflow()})

    assert [lane.step for lane in lanes] == ["cover one", "cover two"], (
        f"both coverage steps must read as lanes, in document order; got "
        f"{[lane.step for lane in lanes]}"
    )
    assert [lane.watchdog for lane in lanes] == [
        pytest.approx(1800.0),
        pytest.approx(2700.0),
    ], "each lane carries the watchdog in force for its own step"


def test_both_lanes_of_one_job_reach_the_ceiling_arithmetic() -> None:
    """The lanes a document yields must sum against that job's ceiling.

    This is the whole point of reading steps rather than jobs, and it
    was only ever checked from lanes constructed in the test. Reading
    the document end to end is what proves a job declaring two coverage
    steps and a one-hour ceiling fails, which is the fault the tiers
    exist to catch.
    """
    lanes = coverage_lanes_of({"ci.yml": _two_step_workflow()})

    grouped = _budgets_per_job(lanes)

    assert list(grouped) == [("ci.yml", "build-test")], (
        f"both steps belong to the one job whose ceiling contains them; got "
        f"{list(grouped)}"
    )
    budgets = [
        lane.watchdog
        for lane in grouped["ci.yml", "build-test"]
        if lane.watchdog is not None
    ]
    ceiling = lanes[0].job_timeout
    assert ceiling is not None, "the synthetic job declares a ceiling"
    assert required_ceiling(budgets) > ceiling, (
        f"a {ceiling:.0f}s ceiling cannot contain {sum(budgets):.0f}s of "
        f"watchdog plus the allowance and the margin; judged one lane at a "
        f"time it would have passed"
    )


def test_two_coverage_steps_in_one_job_are_judged_together() -> None:
    """The ceiling belongs to the job, so its lanes are summed.

    Judging each lane separately against the same ceiling asks only that
    it clear the largest budget. That is the requirement a job running
    the action once happens to satisfy, and it is why this repository's
    two lanes could not tell the two readings apart.
    """
    lanes = (
        CoverageLane(
            workflow="ci.yml",
            job="build-test",
            step="cover one",
            watchdog=1800.0,
            job_timeout=60 * 60.0,
        ),
        CoverageLane(
            workflow="ci.yml",
            job="build-test",
            step="cover two",
            watchdog=2700.0,
            job_timeout=60 * 60.0,
        ),
    )

    grouped = _budgets_per_job(lanes)

    assert list(grouped) == [("ci.yml", "build-test")], (
        "both steps belong to the one job whose ceiling contains them"
    )
    budgets = [
        lane.watchdog
        for lane in grouped["ci.yml", "build-test"]
        if lane.watchdog is not None
    ]
    assert budgets == [1800.0, 2700.0], (
        f"the group must keep every lane of the job, got {budgets}; keeping "
        f"one would ask the ceiling to contain that lane alone"
    )
    ceiling = lanes[0].job_timeout
    assert ceiling is not None, "the synthetic lanes declare a ceiling"
    assert required_ceiling(budgets) > ceiling, (
        "a 60-minute ceiling cannot contain 1,800 s and 2,700 s of watchdog "
        "plus the allowance and the margin; judged one lane at a time it would "
        "have passed"
    )


def test_a_second_coverage_step_carries_its_own_condition() -> None:
    """Two steps in one job are two lanes, each judged separately.

    The condition pin was keyed by workflow and job alone, so the second
    step's entry overwrote the first's. A step skipped by `if: false`
    beside one carrying the expected condition therefore passed
    unexamined, which is the shape this reading has to keep distinct.
    """
    documents = {
        "ci.yml": {
            "jobs": {
                "build-test": {
                    "timeout-minutes": 60,
                    "env": {WATCHDOG_VARIABLE: "1800"},
                    "steps": [
                        {
                            "name": "Coverage",
                            "uses": COVERAGE_STEP,
                            "if": "github.event_name == 'pull_request'",
                        },
                        {
                            "name": "Coverage again",
                            "uses": COVERAGE_STEP,
                            "if": False,
                        },
                    ],
                }
            }
        }
    }

    lanes = coverage_lanes_of(documents)
    keyed = {(lane.workflow, lane.job, lane.step): lane.condition for lane in lanes}

    assert len(keyed) == 2, (
        "two coverage steps in one job are two lanes; keyed by job alone the "
        "second overwrites the first and its condition is never judged"
    )
    assert keyed["ci.yml", "build-test", "Coverage again"] == (False, None), (
        "the skipped step keeps its own condition rather than inheriting the "
        "one its neighbour carries"
    )
