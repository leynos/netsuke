"""The whole-run tier, driven with configurations this repository lacks.

`.config/nextest.toml` sets one ``global-timeout``, so the contract over
it exercises one point of the rule and agrees with every rule that
happens to accept that point. These cases set one above and below each
bound in turn, including none at all, so the branches the repository
never reaches are executed.

``bounds_a_single_test`` is here for the same reason: this file bounds
its default profile, so the reading agrees with one that accepted an
override, or any declaration at all, against the real tree alone.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
from coverage_lanes import CoverageLane
from nextest_budgets import bounds_a_single_test
from timeout_budgets import CAPPED_PROFILE, COLD_BUILD_ALLOWANCE_SECONDS
from whole_run_ordering import watchdog_required_for, whole_run_ordering_faults

#: A default profile bounding one test at 600 s: ten warning periods of
#: sixty seconds. The shape `.config/nextest.toml` uses, with a
#: multiplier of its own so the cases below can sit either side of the
#: bound without tracking the repository's figures.
_BOUNDED_PROFILE: typ.Final[str] = (
    '[profile.default]\nslow-timeout = { period = "60s", terminate-after = 10 }\n'
)


def _config(global_timeout: str | None = None) -> str:
    """Return a configuration bounding one test, with an optional whole run.

    The whole run goes in its own profile, as it does in
    `.config/nextest.toml`: the per-test allowance belongs to `default`,
    which every local run uses, and the whole-run budget to the profile
    CI selects. A case writing both into one table would exercise a
    shape this repository does not have.

    Returns
    -------
    str
        The configuration text.
    """
    if global_timeout is None:
        return _BOUNDED_PROFILE
    return (
        f"{_BOUNDED_PROFILE}\n[profile.{CAPPED_PROFILE}]\n"
        f'global-timeout = "{global_timeout}"\n'
    )


def _lane(watchdog: float | None) -> CoverageLane:
    """Return one coverage lane carrying a watchdog and nothing else."""
    return CoverageLane(
        workflow="ci.yml",
        job="build-test",
        step="Test and Measure Coverage",
        watchdog=watchdog,
        job_timeout=100 * 60.0,
    )


def test_a_configuration_with_no_whole_run_budget_has_no_fault() -> None:
    """There is no tier three to order, so the rule says nothing.

    A repository that has not set the budget has a file that is
    incomplete rather than wrong, and the ordering rule is not what
    reports that: ``timeout_ordering_test`` asserts the key's presence
    separately, and this repository's own file now sets one.
    """
    assert not whole_run_ordering_faults(_config(), [_lane(4200.0)]), (
        "a configuration setting no global-timeout has no tier three, so the "
        "rule must report nothing rather than fail an incomplete file"
    )
    assert watchdog_required_for(_config()) is None, (
        "no whole-run budget means no requirement to derive from it"
    )


def test_the_required_watchdog_carries_all_three_terms() -> None:
    """The whole run, the termination allowance, and a cold build.

    Dropping any one leaves a watchdog that pre-empts the tier below it,
    and each term is small enough beside the others that a rule missing
    one still looks plausible.
    """
    required = watchdog_required_for(_config("40m"))

    assert required == pytest.approx(40 * 60.0 + 70.0 + COLD_BUILD_ALLOWANCE_SECONDS), (
        "the requirement is the whole run, plus nextest's default grace "
        "period and the safety margin, plus the cold-build allowance"
    )


def test_a_whole_run_below_the_per_test_allowance_is_a_fault() -> None:
    """The run would end before the slowest test could use its budget.

    600 s is what this profile allows one test, so a 300 s whole run
    makes the per-test tier unreachable while every value in the file
    still reads as deliberate.
    """
    faults = whole_run_ordering_faults(_config("300s"), [_lane(100_000.0)])

    assert len(faults) == 1, faults
    assert "largest per-test allowance" in faults[0], (
        f"the fault must name the tier it is about; got {faults[0]}"
    )


def test_a_whole_run_equal_to_the_per_test_allowance_is_a_fault() -> None:
    """Equality is not above it, and the boundary is where rules slip.

    A rule written with `>=` passes this case and fails nothing else, so
    the strictness of the comparison is stated rather than implied.
    """
    faults = whole_run_ordering_faults(_config("600s"), [_lane(100_000.0)])

    assert len(faults) == 1, faults
    assert "largest per-test allowance" in faults[0], (
        f"the fault must name the tier it is about; got {faults[0]}"
    )


def test_a_watchdog_below_the_requirement_is_a_fault() -> None:
    """Cargo would be killed before nextest could report the overrun.

    The lane's watchdog is one second short of the requirement, so a
    rule comparing against the whole-run budget alone, without the
    termination and cold-build terms, would pass it.
    """
    config = _config("40m")
    required = watchdog_required_for(config)
    assert required is not None, "a configured global-timeout yields a requirement"

    faults = whole_run_ordering_faults(config, [_lane(required - 1.0)])

    assert len(faults) == 1, faults
    assert "below the" in faults[0], (
        f"the fault must say the watchdog is short; got {faults[0]}"
    )


def test_a_watchdog_meeting_the_requirement_exactly_is_no_fault() -> None:
    """The requirement is a floor, so reaching it is enough.

    Stated beside the case below it because a rule using `>` rather than
    `>=` fails only here.
    """
    config = _config("40m")
    required = watchdog_required_for(config)
    assert required is not None, "a configured global-timeout yields a requirement"

    assert not whole_run_ordering_faults(config, [_lane(required)]), (
        "the requirement is a floor, so a watchdog reaching it is enough"
    )


def test_a_lane_with_no_watchdog_is_a_fault() -> None:
    """An absent watchdog is the action's 1,800 s default, not a pass.

    The lane reads as `None`, which every numeric comparison would have
    raised on or skipped rather than reported.
    """
    faults = whole_run_ordering_faults(_config("40m"), [_lane(None)])

    assert len(faults) == 1, faults
    assert "sets no watchdog" in faults[0], (
        f"the fault must name the absent watchdog; got {faults[0]}"
    )


def test_every_lane_at_fault_is_reported() -> None:
    """Two short lanes are two faults, not the first one found.

    A rule raising on the first lane leaves the second unexamined, so a
    reader fixes one watchdog and returns to the same failure.
    """
    config = _config("40m")

    faults = whole_run_ordering_faults(config, [_lane(1.0), _lane(2.0)])

    assert len(faults) == 2, faults


@pytest.mark.parametrize(
    ("config_text", "expected"),
    [
        pytest.param(_BOUNDED_PROFILE, True, id="the-profile-s-own-table"),
        pytest.param(
            "[profile.default]\n"
            "\n[[profile.default.overrides]]\n"
            "filter = 'binary(slow)'\n"
            'slow-timeout = { period = "60s", terminate-after = 10 }\n',
            False,
            id="only-an-override",
        ),
        pytest.param(
            '[profile.default]\nslow-timeout = { period = "60s" }\n',
            False,
            id="a-table-with-no-terminate-after",
        ),
        pytest.param(
            '[profile.default]\nslow-timeout = "60s"\n',
            False,
            id="a-bare-duration",
        ),
        pytest.param(
            '[profile.other]\nslow-timeout = { period = "60s", '
            "terminate-after = 10 }\n",
            False,
            id="another-profile-s-table",
        ),
    ],
)
def test_only_a_profile_s_own_terminating_table_bounds_its_tests(
    config_text: str, *, expected: bool
) -> None:
    """An override bounds its filter's tests; the profile bounds the rest.

    `.config/nextest.toml` bounds its default profile, so against the
    real tree this reading agrees with one that accepted an override, a
    table with no `terminate-after`, or a bare duration. Each of those
    leaves every test the overrides do not match running with no bound
    at all while `largest_test_allowance` still reports a comfortable
    number.
    """
    assert bounds_a_single_test(config_text) is expected, (
        f"only a profile's own terminating table bounds the tests no override "
        f"matches; this configuration must read as {expected}"
    )
