"""How the workflow reading behaves on workflows written for a case.

Separated from `timeout_budget_properties_test`, whose subject is the
arithmetic over nextest configurations. These drive the lane reading and
the ceiling requirement with synthetic workflows, which is where a job
with two coverage steps, a job with no ceiling and a malformed document
can be described at all: this repository has none of them.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
from coverage_lanes import (
    WatchdogValueError,
    coverage_lanes_of,
    watchdog_of,
)
from timeout_budgets import (
    CEILING_MARGIN_SECONDS,
    OUTSIDE_WATCHDOG_ALLOWANCE_SECONDS,
    WATCHDOG_VARIABLE,
)
from timeout_ordering_test import required_ceiling

COVERAGE_STEP: typ.Final[str] = (
    "leynos/shared-actions/.github/actions/generate-coverage@abc123"
)


def _workflow(**job_fields: object) -> dict[str, dict[str, object]]:
    """Return one synthetic workflow document containing one job.

    Parameters
    ----------
    **job_fields
        Fields to place on the job alongside its coverage step.

    Returns
    -------
    dict
        A document with a single `build` job.
    """
    job: dict[str, object] = {"steps": [{"name": "cover", "uses": COVERAGE_STEP}]}
    job.update(job_fields)
    return {"jobs": {"build": job}}


def test_a_lane_is_read_from_a_synthetic_workflow() -> None:
    """The reading is driven without touching the repository.

    Every assertion over the real tree is satisfied by a reading that
    happens to agree with it on two lanes. Synthetic documents are where
    a wrong field or a missing ceiling shows up as a wrong lane rather
    than as a passing contract.
    """
    documents = {
        "ci.yml": _workflow(**{"timeout-minutes": 60, "env": {WATCHDOG_VARIABLE: 1800}})
    }
    (lane,) = coverage_lanes_of(documents)
    assert lane.workflow == "ci.yml", "the lane carries its file name"
    assert lane.job == "build", "the lane carries its job identifier"
    assert lane.step == "cover", "the lane carries the step's declared name"
    assert lane.watchdog == pytest.approx(1800.0), "the job's watchdog applies"
    assert lane.job_timeout == pytest.approx(3600.0), "minutes convert to seconds"


def test_a_job_without_a_ceiling_reads_as_none_rather_than_absent() -> None:
    """An absent entry would make the ceiling assertion skip the lane.

    That is the failure this contract exists to prevent, so the missing
    ceiling has to survive the reading as a lane with `None` rather than
    disappearing from the list.
    """
    documents = {"ci.yml": _workflow(env={WATCHDOG_VARIABLE: 1800})}
    (lane,) = coverage_lanes_of(documents)
    assert lane.job_timeout is None, "a job with no timeout-minutes reads as None"


def test_a_job_running_no_coverage_step_yields_no_lane() -> None:
    """Only coverage steps are lanes."""
    documents = {"ci.yml": {"jobs": {"build": {"steps": [{"run": "make test"}]}}}}
    assert not coverage_lanes_of(documents), (
        "a job that never invokes the coverage action is not a lane"
    )


@pytest.mark.parametrize(
    "document",
    [
        pytest.param({"jobs": {"build": "not a mapping"}}, id="a-job-that-is-a-scalar"),
        pytest.param({"jobs": {}}, id="no-jobs"),
        pytest.param({}, id="an-empty-document"),
        pytest.param({"jobs": {"build": {"steps": "not a list"}}}, id="steps-as-text"),
    ],
)
def test_a_malformed_workflow_yields_no_lane_rather_than_raising(
    document: dict[str, object],
) -> None:
    """A shape the reading does not expect must not become a lane.

    Raising here would fail the whole contract on an unrelated workflow;
    inventing a lane would assert budgets nobody wrote.
    """
    assert not coverage_lanes_of({"ci.yml": document}), (
        f"{document!r} declares no coverage lane"
    )


@pytest.mark.parametrize(
    ("step", "job", "document", "expected"),
    [
        pytest.param(
            {"env": {WATCHDOG_VARIABLE: 2400}},
            {"env": {WATCHDOG_VARIABLE: 1800}},
            {"env": {WATCHDOG_VARIABLE: 1200}},
            2400.0,
            id="the-step-wins",
        ),
        pytest.param(
            {},
            {"env": {WATCHDOG_VARIABLE: 1800}},
            {"env": {WATCHDOG_VARIABLE: 1200}},
            1800.0,
            id="then-the-job",
        ),
        pytest.param(
            {}, {}, {"env": {WATCHDOG_VARIABLE: 1200}}, 1200.0, id="then-the-workflow"
        ),
        pytest.param({}, {}, {}, None, id="absent-everywhere"),
        pytest.param(
            {"env": "not a mapping"},
            {"env": {WATCHDOG_VARIABLE: 1800}},
            {},
            1800.0,
            id="a-malformed-scope-is-skipped",
        ),
    ],
)
def test_the_watchdog_resolves_innermost_first(
    step: dict[str, object],
    job: dict[str, object],
    document: dict[str, object],
    expected: float | None,
) -> None:
    """Step, then job, then workflow, as GitHub resolves them.

    Both workflows here set the value at job level, so a reading that
    stopped at the job passes on the real tree while missing a
    workflow-level value entirely.
    """
    result = watchdog_of(document, job, step)
    if expected is None:
        assert result is None, "no scope naming the variable must read as absent"
    else:
        assert result == pytest.approx(expected), f"expected {expected}s"


def test_the_required_ceiling_sums_the_watchdogs_and_adds_the_margin() -> None:
    """One job with two coverage steps needs both budgets, plus slack.

    Every job here runs the action once, so the sum and the largest
    budget agree and the assertion over the workflows cannot tell them
    apart. It also sits far enough above its requirement that dropping
    the margin changes nothing observable. Both terms are therefore
    driven with controlled numbers.
    """
    assert required_ceiling([1800.0, 2700.0]) == pytest.approx(
        4500.0 + OUTSIDE_WATCHDOG_ALLOWANCE_SECONDS + CEILING_MARGIN_SECONDS
    ), "two steps need the sum of their budgets, not the larger of them"
    assert required_ceiling([1800.0]) == pytest.approx(
        1800.0 + OUTSIDE_WATCHDOG_ALLOWANCE_SECONDS + CEILING_MARGIN_SECONDS
    ), "one step needs its own budget, the allowance and the margin"
    assert required_ceiling([]) == pytest.approx(
        OUTSIDE_WATCHDOG_ALLOWANCE_SECONDS + CEILING_MARGIN_SECONDS
    ), "the margin is a term of its own, not a fraction of the others"


@pytest.mark.parametrize(
    "value",
    ["abc", "1800s", "", "   ", "0", "-30"],
    ids=["words", "a-unit-suffix", "empty", "spaces", "zero", "negative"],
)
def test_an_unreadable_watchdog_names_the_lane_or_falls_through(value: str) -> None:
    """A value the action cannot read must not stop the run silently.

    `float("abc")` raised before any assertion ran, so the failure was a
    Python fault with no lane in it. A blank value is different again:
    it is what a workflow writes when it interpolates an expression that
    resolved to nothing, so it says nothing and resolution continues.

    Zero and negative are refused rather than returned, because the
    action reads them as no timeout at all: a lane carrying one has no
    third tier while appearing to declare one.
    """
    documents = {
        "ci.yml": {
            "jobs": {
                "build": {
                    "timeout-minutes": 60,
                    "steps": [
                        {
                            "name": "cover",
                            "uses": COVERAGE_STEP,
                            "env": {WATCHDOG_VARIABLE: value},
                        }
                    ],
                }
            }
        }
    }

    if not value.strip():
        (lane,) = coverage_lanes_of(documents)
        assert lane.watchdog is None, (
            "a blank value says nothing, so the lane reads as unset rather "
            "than as a budget of zero"
        )
        return

    with pytest.raises(WatchdogValueError, match=r"ci\.yml:build"):
        coverage_lanes_of(documents)


@pytest.mark.parametrize(
    "blank",
    [pytest.param("", id="empty"), pytest.param("   ", id="whitespace-only")],
)
def test_a_blank_step_value_falls_through_to_the_scope_outside_it(
    blank: str,
) -> None:
    """A blank value says nothing, so the next scope decides.

    That is what a workflow writes when it interpolates an expression
    that resolved to nothing, and the lane is then bounded by whatever
    the job or the workflow set. A reading that returned None as soon as
    the step's own value was blank would report the lane as unset and
    apply the action's undocumented default in place of the budget the
    job actually declares.

    The earlier blank case has no outer value to fall through to, so it
    passes against that mistaken reading as well as the right one.
    """
    documents = {
        "ci.yml": {
            "env": {WATCHDOG_VARIABLE: "900"},
            "jobs": {
                "build-test": {
                    "timeout-minutes": 60,
                    "env": {WATCHDOG_VARIABLE: "1800"},
                    "steps": [
                        {
                            "name": "Coverage",
                            "uses": COVERAGE_STEP,
                            "env": {WATCHDOG_VARIABLE: blank},
                        }
                    ],
                }
            },
        }
    }

    (lane,) = coverage_lanes_of(documents)
    assert lane.watchdog == pytest.approx(1800.0), (
        "a blank step value must fall through to the job's, not read as unset; "
        "reading it as unset would apply the action's default instead of the "
        "1,800 s the job declares"
    )


@pytest.mark.parametrize(
    "value",
    [
        pytest.param("nan", id="not-a-number"),
        pytest.param("inf", id="infinity"),
        pytest.param("-inf", id="negative-infinity"),
    ],
)
def test_a_non_finite_watchdog_is_refused(value: str) -> None:
    """`float` accepts these, and `<= 0` does not catch them.

    `float("nan") <= 0` is False and so is `float("inf") <= 0`, so both
    passed the positivity check and reached the ceiling arithmetic,
    where the comparison against a finite ceiling fails for a reason
    that names the ceiling rather than the value at fault.
    """
    documents = {
        "ci.yml": {
            "jobs": {
                "build-test": {
                    "timeout-minutes": 60,
                    "steps": [
                        {
                            "name": "Coverage",
                            "uses": COVERAGE_STEP,
                            "env": {WATCHDOG_VARIABLE: value},
                        }
                    ],
                }
            }
        }
    }

    with pytest.raises(WatchdogValueError, match=r"finite"):
        coverage_lanes_of(documents)
