"""The tier comparisons stay exact all the way from the files they read.

``nextest_duration_test`` proves that a nextest duration is read exactly.
That is only half of it. Every tier comparison is a sum, and each sum
mixes a duration read from ``.config/nextest.toml`` with a budget read
from a workflow and with a constant declared in ``timeout_budgets``. A
sum is exact only if every term is: one ``float`` among them converts the
whole of it back, and the conversion is silent.

So this module drives the three compositions the ordering contract
actually evaluates, each with two inputs one second apart and large
enough that a float cannot hold both. The repository's own budgets are
nowhere near that magnitude and never will be, which is precisely why
the loss cannot be exposed by the real files: a contract resting on them
would pass with every term a float.

Each case asserts the float collapse alongside the exact comparison. The
collapse is the thing being avoided rather than an incidental detail, and
a case that stopped exercising it would keep passing while proving
nothing.
"""

import fractions
import typing as typ

import pytest
from coverage_lanes import coverage_lanes_of
from nextest_budgets import global_timeout, termination_allowance
from nextest_durations import seconds
from timeout_budgets import (
    CAPPED_PROFILE,
    CEILING_MARGIN_SECONDS,
    COLD_BUILD_ALLOWANCE_SECONDS,
    COVERAGE_ACTION,
    NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS,
    OUTSIDE_WATCHDOG_ALLOWANCE_SECONDS,
    TERMINATION_SAFETY_MARGIN_SECONDS,
    WATCHDOG_VARIABLE,
    required_ceiling,
)
from whole_run_ordering import watchdog_required_for

#: A magnitude whose neighbouring floats are 256 seconds apart, so a
#: one-second difference is lost outright rather than only sometimes.
#: Two to the fifty-third is the first magnitude that loses anything, but
#: there the spacing is two and whether a given pair collapses depends on
#: which side of a tie it falls; at two to the sixtieth every difference
#: below 128 seconds vanishes, which is what these cases need.
HUGE_SECONDS: typ.Final[int] = 2**60


def _workflow(watchdog: int, timeout_minutes: int) -> dict[str, dict[str, object]]:
    """Return one document declaring a single coverage job."""
    # The watchdog sits at job level because both of this repository's
    # workflows declare it there.
    return {
        "ci.yml": {
            "jobs": {
                "coverage": {
                    "timeout-minutes": timeout_minutes,
                    "env": {WATCHDOG_VARIABLE: watchdog},
                    "steps": [{"uses": f"{COVERAGE_ACTION}@" + "0" * 40}],
                }
            }
        }
    }


def _watchdog_read(watchdog: int) -> fractions.Fraction:
    """Return the watchdog budget the lane reader takes from a document."""
    (lane,) = coverage_lanes_of(_workflow(watchdog, timeout_minutes=65))
    assert lane.watchdog is not None, "the job declares a watchdog at job level"
    return lane.watchdog


def _job_ceiling_read(timeout_minutes: int) -> fractions.Fraction:
    """Return the job ceiling in seconds, converted from its minutes."""
    (lane,) = coverage_lanes_of(_workflow(1800, timeout_minutes=timeout_minutes))
    assert lane.job_timeout is not None, "the job declares timeout-minutes"
    return lane.job_timeout


def _assert_orders_strictly(
    larger: fractions.Fraction, smaller: fractions.Fraction, what: str
) -> None:
    """Assert an exact ordering, and that a float would have lost it."""
    # The second assertion keeps the first honest. These cases exist to
    # catch a term reverting to a float, so one whose two inputs stopped
    # colliding as floats would pass without exercising anything, and
    # would go on passing after the defect returned.
    assert larger > smaller, f"{what} must order strictly"
    # RUF069 is right in general and wrong here: comparing two floats
    # for equality is the assertion, not an oversight. These two must
    # collide as floats or the case above is not exercising the loss it
    # was written to catch.
    assert float(larger) == float(smaller), (  # ruff: ignore[float-equality-comparison]
        f"the float collapse this guards against must still be real for "
        f"{what}; if these differ, the case no longer exercises what it was "
        f"written for"
    )


def test_the_duration_reader_keeps_a_second_a_float_would_lose() -> None:
    """Two budgets one second apart must not read as one value.

    The first term of every composition below is a duration out of
    ``.config/nextest.toml``. Both of these are inputs in the estate
    differential, so this is not a magnitude invented for the contract.
    """
    _assert_orders_strictly(
        seconds(f"{HUGE_SECONDS + 1}s"),
        seconds(f"{HUGE_SECONDS}s"),
        "durations one second apart",
    )


def test_the_lane_reader_keeps_a_second_a_float_would_lose() -> None:
    """Two watchdog budgets one second apart must not read as one value.

    The workflow side was read with ``float(text)``, so the exactness
    won on the nextest side would be discarded the moment a watchdog
    entered the comparison. Both the job ceiling and the watchdog floor
    compare against a workflow value, so both inherit it.
    """
    _assert_orders_strictly(
        _watchdog_read(HUGE_SECONDS + 1),
        _watchdog_read(HUGE_SECONDS),
        "watchdogs one second apart",
    )


def test_the_job_ceiling_reader_keeps_a_minute_a_float_would_lose() -> None:
    """Two ceilings one minute apart must not read as one value.

    The ceiling is converted as well as read: ``timeout-minutes`` is
    multiplied by sixty. Doing either in floating point loses the
    comparison that holds a job above the sum of its watchdogs, so a
    ceiling a minute short of that sum would be accepted.
    """
    _assert_orders_strictly(
        _job_ceiling_read(HUGE_SECONDS + 1),
        _job_ceiling_read(HUGE_SECONDS),
        "ceilings one minute apart",
    )


def test_the_job_ceiling_arithmetic_stays_exact() -> None:
    """The first composition: the watchdogs, the outside work, the margin.

    ``required_ceiling`` adds a job's watchdog budgets, the measured work
    outside them and the margin above that sum. Either allowance being a
    float makes the total a float whatever the budgets are, and the
    comparison that reads it would then accept a job ceiling a second
    short of containing its own watchdogs.
    """
    _assert_orders_strictly(
        required_ceiling([fractions.Fraction(HUGE_SECONDS + 1)]),
        required_ceiling([fractions.Fraction(HUGE_SECONDS)]),
        "required ceilings one second apart",
    )


def _config(whole_run: int, *, grace: bool = True, profile: str = "default") -> str:
    """Return a configuration declaring one profile's tiers.

    The profile is a parameter because the readers disagree about which
    one they mean. ``global_timeout`` defaults to the profile CI selects,
    while a caller naming ``default`` is reading the table nextest
    inherits from. A case driving the one through a configuration
    declaring the other finds no budget at all.

    Returns
    -------
    str
        The configuration text, declaring the named profile alone.
    """
    # Both spellings of the grace period are driven: without a declared
    # one nextest's default applies, and that default is a term of the
    # allowance like any other.
    declared = ', grace-period = "5s"' if grace else ""
    return (
        f"[profile.{profile}]\n"
        f'global-timeout = "{whole_run}s"\n'
        f'slow-timeout = {{ period = "60s", terminate-after = 1{declared} }}\n'
    )


def test_the_termination_allowance_stays_exact() -> None:
    """The second composition: a grace period plus the safety margin.

    Two terms, and the margin is the one that was a float. The grace
    period is the term that moves here, so the case fails if either the
    duration reaching it or the margin added to it stops being exact.

    Nextest's own default is the other spelling of the first term, and
    it cannot be driven by an ordering because nothing about it varies.
    It is a term of the watchdog floor below, where the whole-run budget
    supplies the movement, and it is named in
    ``test_every_constant_the_compositions_add_is_exact``.
    """
    larger = _config(1, grace=False).replace(
        "terminate-after = 1",
        f'terminate-after = 1, grace-period = "{HUGE_SECONDS + 1}s"',
    )
    smaller = _config(1, grace=False).replace(
        "terminate-after = 1", f'terminate-after = 1, grace-period = "{HUGE_SECONDS}s"'
    )
    _assert_orders_strictly(
        termination_allowance(larger),
        termination_allowance(smaller),
        "termination allowances one second apart",
    )


def _watchdog_floor(whole_run: int, *, grace: bool = True) -> fractions.Fraction:
    """Return the watchdog floor, rebuilt from the terms it sums.

    Deliberately not a call to ``watchdog_required_for``. Rebuilding the
    sum means a term reverting to a float fails against the term rather
    than against the function, so the report names which input lost its
    exactness. ``test_the_watchdog_floor_function_stays_exact`` drives
    the function itself, which is what the ordering contract calls, and
    is what catches a cast applied inside it rather than to one of its
    inputs.

    Returns
    -------
    fractions.Fraction
        The whole-run budget, the termination allowance and the cold
        build allowance, summed exactly.
    """
    config_text = _config(whole_run, grace=grace)
    budget = global_timeout(config_text, profile="default")
    assert budget is not None, "the profile declares a global-timeout"
    return budget + termination_allowance(config_text) + COLD_BUILD_ALLOWANCE_SECONDS


def _watchdog_floor_of(whole_run: int) -> fractions.Fraction:
    """Return what ``watchdog_required_for`` derives for one configuration.

    The configuration declares the profile CI selects, because that is
    the one the function reads. Declaring ``default`` instead yields no
    budget and the function returns None, which is the shape of a case
    that asserts nothing rather than one that fails.

    Returns
    -------
    fractions.Fraction
        The watchdog floor the function derives for that configuration.
    """
    floor = watchdog_required_for(_config(whole_run, profile=CAPPED_PROFILE))
    assert floor is not None, "the profile declares a global-timeout"
    return floor


def test_the_watchdog_floor_function_stays_exact() -> None:
    """``watchdog_required_for`` itself keeps a second a float would lose.

    The composition cases below rebuild this sum from its terms, which
    localises a lossy input but leaves the function the ordering
    contract actually calls unexercised at a magnitude that can see the
    loss. A ``float`` applied inside ``watchdog_required_for``, to its
    result or to any term as it is added, would therefore pass every
    other case here. This one refuses it.
    """
    _assert_orders_strictly(
        _watchdog_floor_of(HUGE_SECONDS + 1),
        _watchdog_floor_of(HUGE_SECONDS),
        "watchdog floors one second apart, through the public function",
    )


@pytest.mark.parametrize(
    "grace",
    [
        pytest.param(True, id="a-declared-grace-period"),
        pytest.param(False, id="nextests-own-default"),
    ],
)
def test_the_watchdog_floor_stays_exact(*, grace: bool) -> None:
    """The third composition: the whole run, the allowance, the cold build.

    Three terms, two of them constants. Either constant being a float
    makes the floor a float even though the whole-run budget reaching it
    is exact, and a watchdog a second below the run it must cover would
    then compare equal to one that covers it.
    """
    _assert_orders_strictly(
        _watchdog_floor(HUGE_SECONDS + 1, grace=grace),
        _watchdog_floor(HUGE_SECONDS, grace=grace),
        f"watchdog floors one second apart (grace-period declared: {grace})",
    )


def test_the_configured_values_arrive_exact() -> None:
    """The real files read exactly too, not merely the constructed ones.

    The cases above use magnitudes this repository will never configure,
    which is what makes them able to see the loss at all. This one is the
    other half: the values actually in force arrive as exact numbers, so
    a float reintroduced anywhere on the live path is caught without
    waiting for a budget nobody will ever set.
    """
    lanes = coverage_lanes_of()
    assert lanes, "the repository declares at least one coverage lane"
    assert isinstance(seconds("600s"), fractions.Fraction), (
        "a nextest duration must arrive exact"
    )
    assert all(
        lane.watchdog is None or isinstance(lane.watchdog, fractions.Fraction)
        for lane in lanes
    ), "every workflow watchdog must arrive exact"
    assert all(
        lane.job_timeout is None or isinstance(lane.job_timeout, fractions.Fraction)
        for lane in lanes
    ), "every job ceiling must arrive exact through its conversion to seconds"
    assert isinstance(
        required_ceiling([fractions.Fraction(1800)]), fractions.Fraction
    ), "the required ceiling must stay exact through its sum"


def test_every_constant_the_compositions_add_is_exact() -> None:
    """Each constant is a term, so each is exact in its own right.

    Named one by one rather than asserted over a collection, so a
    constant reverting to a float fails with its own name in the report
    rather than as a count.
    """
    for name, value in (
        ("OUTSIDE_WATCHDOG_ALLOWANCE_SECONDS", OUTSIDE_WATCHDOG_ALLOWANCE_SECONDS),
        ("CEILING_MARGIN_SECONDS", CEILING_MARGIN_SECONDS),
        ("COLD_BUILD_ALLOWANCE_SECONDS", COLD_BUILD_ALLOWANCE_SECONDS),
        ("TERMINATION_SAFETY_MARGIN_SECONDS", TERMINATION_SAFETY_MARGIN_SECONDS),
        (
            "NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS",
            NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS,
        ),
    ):
        assert isinstance(value, fractions.Fraction), (
            f"{name} is a term of a tier comparison and must be exact"
        )
