"""Unit and property coverage for the timeout readings.

The ordering contract compares four numbers, and its assertions over the
repository's own files are satisfied by several plausibly wrong readings:
every `terminate-after` here is 5 or 10 and every period is 60 s, so a
reading that confused the two would still order the tiers correctly. The
tests below drive the readings with synthetic configurations and
synthetic workflows instead, where a wrong reading has nowhere to hide.

Error paths are covered as well. A reading that guessed at a malformed
configuration would put the ordering assertions against a budget nextest
never applies, and they would pass.
"""

import typing as typ

import pytest
from coverage_lanes import coverage_lanes_of, watchdog_of
from hypothesis import given
from hypothesis import strategies as st
from timeout_budgets import (
    NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS,
    TERMINATION_SAFETY_MARGIN_SECONDS,
    WATCHDOG_VARIABLE,
    global_timeout,
    grace_period,
    largest_test_allowance,
    seconds,
    termination_allowance,
)

#: The units nextest accepts, with their length in seconds.
UNITS: typ.Final[dict[str, float]] = {"ms": 0.001, "s": 1.0, "m": 60.0, "h": 3600.0}

COVERAGE_STEP: typ.Final[str] = (
    "leynos/shared-actions/.github/actions/generate-coverage@abc123"
)

whole_numbers = st.integers(min_value=1, max_value=10_000)
units = st.sampled_from(sorted(UNITS))
multipliers = st.integers(min_value=1, max_value=20)


@pytest.mark.parametrize(
    ("duration", "expected"),
    [
        pytest.param("500ms", 0.5, id="milliseconds"),
        pytest.param("90s", 90.0, id="seconds"),
        pytest.param("15m", 900.0, id="minutes"),
        pytest.param("2h", 7200.0, id="hours"),
        pytest.param("  45s  ", 45.0, id="surrounding-whitespace"),
        pytest.param("1.5m", 90.0, id="a-decimal-value"),
    ],
)
def test_each_unit_converts_exactly(duration: str, expected: float) -> None:
    """The unit table decides every comparison the contract makes.

    A single wrong entry would leave every downstream assertion an
    inequality between two plausible numbers, so each unit is pinned
    rather than sampled.
    """
    assert seconds(duration) == pytest.approx(expected), (
        f"{duration!r} must convert to {expected}s"
    )


@given(value=whole_numbers, unit=units)
def test_every_unit_scales_its_value(value: int, unit: str) -> None:
    """A duration is its number times the length of its unit."""
    assert seconds(f"{value}{unit}") == pytest.approx(value * UNITS[unit]), (
        f"{value}{unit} must scale by the length of its unit"
    )


@pytest.mark.parametrize(
    "duration",
    ["", "300", "s", "300 sec", "five minutes", "-30s", "30d", "3 0s"],
    ids=[
        "empty",
        "no-unit",
        "no-value",
        "an-unsupported-spelling",
        "words",
        "negative",
        "days-are-not-a-nextest-unit",
        "an-interior-space",
    ],
)
def test_an_unreadable_duration_is_refused(duration: str) -> None:
    """A duration nextest would reject must not become a number.

    Returning something plausible for `"30d"` would put a comparison
    against a budget nextest never applies, and the contract would pass
    while the ordering it claims to hold did not.
    """
    with pytest.raises(AssertionError):
        seconds(duration)


def test_the_repository_s_own_per_test_budgets_are_the_documented_ones() -> None:
    """300 s ordinarily, 600 s for the two Windows tests.

    Pinned rather than derived, because the developers' guide states
    these two figures and nothing else would notice them drifting.
    """
    config = (
        '[profile.default]\nslow-timeout = { period = "60s", terminate-after = 5 }\n'
        "\n[[profile.default.overrides]]\n"
        'slow-timeout = { period = "60s", terminate-after = 10 }\n'
    )
    assert largest_test_allowance(config) == pytest.approx(600.0), (
        "ten warning periods of sixty seconds is a 600s budget"
    )
    base_only = (
        '[profile.default]\nslow-timeout = { period = "60s", terminate-after = 5 }\n'
    )
    assert largest_test_allowance(base_only) == pytest.approx(300.0), (
        "five warning periods of sixty seconds is a 300s budget"
    )


@given(
    budgets=st.lists(
        st.tuples(whole_numbers, units, multipliers), min_size=1, max_size=8
    )
)
def test_the_largest_budget_is_the_largest_product(
    budgets: list[tuple[int, str, int]],
) -> None:
    """Every `slow-timeout` counts, and each counts as a product.

    A reading that took the first entry, or the largest period without
    its multiplier, agrees with the hand-written cases whenever they
    coincide. Over generated configurations they stop coinciding.
    """
    config = "\n".join(
        f'slow-timeout = {{ period = "{value}{unit}", terminate-after = {times} }}'
        for value, unit, times in budgets
    )
    expected = max(value * UNITS[unit] * times for value, unit, times in budgets)
    assert largest_test_allowance(config) == pytest.approx(expected), (
        "the largest budget is the largest period times its own multiplier"
    )


def test_a_slow_timeout_without_a_multiplier_counts_once() -> None:
    """An omitted `terminate-after` is one period, not none."""
    assert largest_test_allowance('slow-timeout = { period = "90s" }') == pytest.approx(
        90.0
    ), "an absent terminate-after means a single period"


def test_a_grace_period_is_not_read_as_a_per_test_budget() -> None:
    """The two keys sit in the same inline table.

    A matcher reading `period` as a substring would take a grace period
    for a per-test budget whenever the former were the larger.
    """
    config = 'slow-timeout = { period = "30s", grace-period = "30m" }'
    assert largest_test_allowance(config) == pytest.approx(30.0), (
        "the per-test reading took a grace period for a slow-timeout"
    )


@pytest.mark.parametrize(
    "config",
    ["", "[profile.default]\nfail-fast = false\n", "slow-timeout = { }"],
    ids=["empty", "no-slow-timeout", "an-empty-table"],
)
def test_a_configuration_with_no_readable_budget_is_refused(config: str) -> None:
    """Returning zero would make every whole-run budget look comfortable."""
    with pytest.raises(AssertionError):
        largest_test_allowance(config)


@pytest.mark.parametrize(
    ("config", "expected"),
    [
        pytest.param("", None, id="absent"),
        pytest.param('global-timeout = "45m"', 2700.0, id="present"),
        pytest.param(
            '[profile.default]\nglobal-timeout = "600s"\n', 600.0, id="inside-a-profile"
        ),
        pytest.param('# global-timeout = "45m"\n', None, id="commented-out-is-not-set"),
    ],
)
def test_the_whole_run_budget_is_read_or_reported_absent(
    config: str, expected: float | None
) -> None:
    """Tier two is absent here, so both branches need proving.

    The contract skips its ordering assertion when the budget is absent.
    If the reading returned a number for a commented-out line, that skip
    would turn into a comparison against a budget nextest never applies.
    """
    result = global_timeout(config)
    if expected is None:
        assert result is None, f"{config!r} sets no global-timeout"
    else:
        assert result == pytest.approx(expected), f"{config!r} sets {expected}s"


@given(periods=st.lists(st.tuples(whole_numbers, units), min_size=1, max_size=6))
def test_the_termination_allowance_tracks_the_largest_grace_period(
    periods: list[tuple[int, str]],
) -> None:
    """The allowance is the largest grace period plus the fixed margin."""
    config = "\n".join(
        f'slow-timeout = {{ period = "1s", grace-period = "{value}{unit}" }}'
        for value, unit in periods
    )
    largest = max(value * UNITS[unit] for value, unit in periods)
    assert grace_period(config) == pytest.approx(largest), (
        "the largest configured grace period governs"
    )
    assert termination_allowance(config) == pytest.approx(
        largest + TERMINATION_SAFETY_MARGIN_SECONDS
    ), "the allowance is the grace period plus the margin, not the larger"


@given(text=st.text(max_size=40).filter(lambda body: "grace-period" not in body))
def test_an_unconfigured_grace_period_falls_back_to_nextest_s_default(
    text: str,
) -> None:
    """Assuming zero would understate what nextest needs to stop a run."""
    assert grace_period(text) == pytest.approx(NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS), (
        "an absent grace period must fall back to nextest's default"
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
