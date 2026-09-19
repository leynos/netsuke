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

import fractions
import typing as typ

import pytest
from coverage_lanes import (
    CoverageLane,
    conditions_by_coordinate,
    coverage_lanes_of,
)
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
            watchdog=fractions.Fraction(1800),
            job_timeout=fractions.Fraction(60 * 60),
        ),
        CoverageLane(
            workflow="ci.yml",
            job="build-test",
            step="cover two",
            watchdog=fractions.Fraction(2700),
            job_timeout=fractions.Fraction(60 * 60),
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


def _two_unnamed_steps_workflow() -> dict[str, dict[str, object]]:
    """Return a job whose two coverage steps declare no name.

    Returns
    -------
    dict
        A document whose `build-test` job runs the action twice, one
        step skipped by `if: false`.
    """
    return {
        "jobs": {
            "build-test": {
                "timeout-minutes": 60,
                "env": {WATCHDOG_VARIABLE: 1800},
                "steps": [
                    {"uses": COVERAGE_STEP},
                    {"uses": COVERAGE_STEP, "if": "false"},
                ],
            }
        }
    }


def test_two_unnamed_coverage_steps_share_a_coordinate() -> None:
    """A coordinate holding two lanes keeps both conditions.

    An unnamed step takes its job's name, so both lanes here read as
    `("ci.yml", "build-test", "build-test")`. A mapping from coordinate
    to a single condition kept only the last, so the step skipped by
    `if: false` beside one carrying the expected condition passed the
    ordering contract unexamined.
    """
    lanes = coverage_lanes_of({"ci.yml": _two_unnamed_steps_workflow()})

    found = conditions_by_coordinate(lanes)

    coordinate = ("ci.yml", "build-test", "build-test")
    expected = {coordinate: ((None, None), ("false", None))}
    assert found == expected, (
        f"both lanes must survive the grouping, in document order; got {found}"
    )


def _doctests_workflow(doctests: object) -> dict[str, dict[str, object]]:
    """Return a workflow whose one coverage step sets the doctests input.

    Returns
    -------
    dict
        A document with a single `build-test` job of one coverage step.
    """
    return {
        "jobs": {
            "build-test": {
                "timeout-minutes": 90,
                "env": {WATCHDOG_VARIABLE: 1800},
                "steps": [
                    {
                        "name": "cover",
                        "uses": COVERAGE_STEP,
                        "with": {"doctests": doctests},
                    }
                ],
            }
        }
    }


def test_a_doctests_step_arms_the_watchdog_twice() -> None:
    """One step, two `cargo` invocations, two watchdog windows.

    The shared action runs `cargo llvm-cov nextest` and then an
    uninstrumented `cargo test --doc` when its `doctests` input asks for
    it, and each invocation arms the watchdog separately. Reading one
    lane for such a step understates what the job's ceiling has to
    contain by a whole watchdog window, which is the sizing fault issue
    715 records. Both lanes carry the same coordinate, because they are
    one step: what the second one adds is the second window.
    """
    lanes = coverage_lanes_of({"ci.yml": _doctests_workflow("true")})

    assert [lane.step for lane in lanes] == ["cover", "cover"], (
        f"a doctests step arms two windows, and both belong to its one "
        f"coordinate; got {[lane.step for lane in lanes]}"
    )
    assert all(lane.watchdog == pytest.approx(1800.0) for lane in lanes), (
        f"both windows run under the same resolved watchdog, got "
        f"{[lane.watchdog for lane in lanes]}"
    )
    first, second = lanes
    assert first == second, (
        f"the two windows of one step share every field, so a contract "
        f"reading them apart still finds one step; got {first} and {second}"
    )


@pytest.mark.parametrize(
    "doctests",
    [
        pytest.param(None, id="no-input-declared"),
        pytest.param("false", id="declined"),
        pytest.param("1", id="truthy-to-the-action-but-not-read-here"),
        pytest.param(True, id="a-yaml-boolean"),
    ],
)
def test_a_step_that_declines_the_doctests_arms_one_window(
    doctests: object,
) -> None:
    """Only the one spelling this repository writes counts here.

    A step that omits the input, sets `false`, or sets a YAML boolean
    runs the instrumented pass alone and reads as one window. The
    `'1'` case is the documented gap rather than a claim about the
    action: `1` is truthy to the action's own reader, so such a step
    would run the doctest pass and this reading would understate its
    windows. The value is held to `'true'` by the input pin in
    `test_execution_coverage_test`, so the gap needs a producer written
    in another spelling to matter at all, and that producer fails the
    pin before it reaches here.
    """
    # Annotated because the input mapping is added to the step below,
    # after the list literal has been typed; inferring `dict[str, str]`
    # from the literal alone rejects the nested mapping.
    steps: list[dict[str, object]] = [{"name": "cover", "uses": COVERAGE_STEP}]
    if doctests is not None:
        steps[0]["with"] = {"doctests": doctests}
    workflow = {"jobs": {"build-test": {"timeout-minutes": 90, "steps": steps}}}

    lanes = coverage_lanes_of({"ci.yml": workflow})

    assert len(lanes) == 1, (
        f"`doctests: {doctests!r}` does not run the doctest pass, so the step "
        f"arms one watchdog window; got {len(lanes)} lanes"
    )


def test_two_windows_from_one_doctests_step_reach_the_ceiling_arithmetic() -> None:
    """The ceiling of the job holding such a step must cover both.

    Two windows of the same 1,800 s watchdog, the measured work outside
    them and the margin above that sum is 5,400 s, so the ninety-minute
    ceiling this repository sets has 3,600 s of slack and the sixty
    minutes it used to set, falling short of the required. A contract
    reading one window would have called the sixty minutes sufficient.
    """
    lanes = coverage_lanes_of({"ci.yml": _doctests_workflow("true")})

    grouped = _budgets_per_job(lanes)
    assert list(grouped) == [("ci.yml", "build-test")], (
        f"both windows belong to the one job whose ceiling contains them; got "
        f"{list(grouped)}"
    )
    budgets = [
        lane.watchdog for lane in grouped["ci.yml", "build-test"] if lane.watchdog
    ]
    required = required_ceiling(budgets)

    assert required == fractions.Fraction(2 * 1800 + 900 + 900), (
        f"two 1,800 s windows, fifteen minutes of work outside them and the "
        f"fifteen-minute margin are 5,400 s; got {required:.0f}s"
    )
    assert required > fractions.Fraction(60 * 60), (
        "a sixty-minute ceiling cannot contain two 1,800 s windows, the work "
        "outside them and the margin; one window made it look sufficient"
    )
