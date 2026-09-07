"""Contract for the timers that can end a test run.

Four independent budgets can end a coverage lane, each set somewhere
different, and they only work if each sits above the one inside it.
Three of the four are set here: a per-test ``slow-timeout`` in
``.config/nextest.toml``, the shared coverage action's wall-clock
watchdog on the ``cargo`` invocation, and the job's own
``timeout-minutes``.

The second tier, nextest's whole-run ``global-timeout``, is not set.
That is a gap rather than a decision: this repository does run nextest,
so the budget exists to be set, and until it is the watchdog is doing
tier two's job as well as its own. A run whose tests each stay inside
their 600 s allowance can still exceed the watchdog between them, and
the failure then names ``cargo`` rather than the run. The contract
therefore binds the value if it appears, so that adding one later lands
in the right place rather than merely somewhere.

The per-test allowance is ``period`` multiplied by ``terminate-after``,
not ``period`` alone. Reading the period as the budget would understate
the largest allowance here fivefold on Linux and tenfold on Windows.

See "Test timeouts: the tiers this repository sets" in
``docs/developers-guide.md``, and the canonical wording in
`leynos/shared-actions`' `generate-coverage` README.

Run via ``make test-workflow-contracts``.
"""

import re

import pytest
from timeout_budgets import (
    COLD_BUILD_ALLOWANCE_SECONDS,
    COVERAGE_ACTION,
    NEXTEST_CONFIG,
    OUTSIDE_WATCHDOG_ALLOWANCE_SECONDS,
    WATCHDOG_VARIABLE,
    CoverageLane,
    coverage_lanes_of,
    global_timeout,
    largest_test_allowance,
    seconds,
    termination_allowance,
)


@pytest.fixture(scope="module")
def nextest_config() -> str:
    """Return the nextest configuration file's text.

    Returns
    -------
    str
        The file's contents.
    """
    return NEXTEST_CONFIG.read_text(encoding="utf-8")


@pytest.fixture(scope="module")
def coverage_lanes() -> tuple[CoverageLane, ...]:
    """Return every step invoking the coverage action, with its budgets.

    Returns
    -------
    tuple[CoverageLane, ...]
        One entry per coverage step.
    """
    return coverage_lanes_of()


def test_the_coverage_action_is_invoked_somewhere(
    coverage_lanes: tuple[CoverageLane, ...],
) -> None:
    """The contract needs a lane to assert against.

    A repin or a rename that stopped the coordinate matching would
    otherwise turn every assertion below into a vacuous pass over an
    empty list, and the loss would look exactly like success.
    """
    assert coverage_lanes, (
        f"no workflow step uses {COVERAGE_ACTION}; either coverage moved or "
        f"this contract stopped recognizing it"
    )


def test_every_coverage_lane_sets_the_watchdog_explicitly(
    coverage_lanes: tuple[CoverageLane, ...],
) -> None:
    """The default is invisible, so every lane must write it down.

    The action kills `cargo` after 1,800 s unless told otherwise, and
    nothing in this repository would mention that if a job stopped
    setting the variable. The value here happens to equal the default,
    which makes writing it down more important rather than less: an
    accidental deletion would change nothing observable until the run it
    killed.
    """
    missing = [str(lane) for lane in coverage_lanes if lane.watchdog is None]
    assert not missing, (
        f"these coverage lanes do not set {WATCHDOG_VARIABLE} and so inherit "
        f"the shared action's undocumented 1,800 s default: {missing}"
    )


def test_the_job_ceiling_covers_the_watchdog_and_the_work_around_it(
    coverage_lanes: tuple[CoverageLane, ...],
) -> None:
    """Tier four must not pre-empt tier three.

    The two clocks do not start together. The job timer starts when the
    job starts, before the checkout, the toolchain setup and the linting
    that precede coverage, and it is still running through whatever
    follows. The watchdog starts when `cargo` does. A ceiling merely
    above the watchdog still cancels the job before the watchdog can
    report an overrun, and a cancellation discards the log that would
    have explained it.
    """
    for lane in coverage_lanes:
        assert lane.watchdog is not None, str(lane)
        assert lane.job_timeout is not None, (
            f"{lane} runs cargo under a {lane.watchdog:.0f}s watchdog in a job "
            f"with no timeout-minutes; the outermost tier is missing and "
            f"GitHub's six-hour default applies"
        )
        required = lane.watchdog + OUTSIDE_WATCHDOG_ALLOWANCE_SECONDS
        assert lane.job_timeout >= required, (
            f"{lane} has a job ceiling of {lane.job_timeout:.0f}s, below the "
            f"{required:.0f}s needed to cover its {lane.watchdog:.0f}s watchdog "
            f"plus {OUTSIDE_WATCHDOG_ALLOWANCE_SECONDS:.0f}s of measured work "
            f"outside it; an overrun would be cancelled rather than reported"
        )


def test_a_whole_run_budget_would_sit_inside_the_watchdog(
    coverage_lanes: tuple[CoverageLane, ...], nextest_config: str
) -> None:
    """Tier three must not pre-empt tier two, if tier two appears.

    No ``global-timeout`` is set today, so this asserts nothing about the
    current tree and is not a licence to leave it that way: the guide
    records the gap. What it does is bind the value the moment one is
    added, so it arrives above the largest per-test allowance and inside
    the watchdog rather than merely somewhere.
    """
    whole_run = global_timeout(nextest_config)
    if whole_run is None:
        pytest.skip("no global-timeout is set; the guide records this as a gap")
    largest = largest_test_allowance(nextest_config)
    assert whole_run > largest, (
        f"the {whole_run:.0f}s global-timeout is not above the {largest:.0f}s "
        f"largest per-test allowance; the run would end before that test "
        f"could use its budget"
    )
    required = (
        whole_run + termination_allowance(nextest_config) + COLD_BUILD_ALLOWANCE_SECONDS
    )
    for lane in coverage_lanes:
        assert lane.watchdog is not None, str(lane)
        assert lane.watchdog >= required, (
            f"{lane} sets a {lane.watchdog:.0f}s watchdog, below the "
            f"{required:.0f}s needed to cover the {whole_run:.0f}s whole-run "
            f"budget, nextest's termination procedure, and a cold build; "
            f"cargo would be killed before nextest could report the overrun"
        )


def test_the_largest_per_test_allowance_counts_the_multiplier(
    nextest_config: str,
) -> None:
    """``terminate-after`` scales the period; the budget is their product.

    This is the reading that decides every comparison above, and it is
    the one easy to get wrong: the periods here are all 60 s, so a
    contract reading the period alone would report a 60 s largest
    allowance where the real figure is 600 s.
    """
    largest = largest_test_allowance(nextest_config)
    periods = [
        seconds(match[1])
        for match in re.finditer(r'period\s*=\s*"([^"]+)"', nextest_config)
    ]
    assert largest > max(periods), (
        f"the largest per-test allowance came out as {largest:.0f}s, no more "
        f"than the longest bare period; terminate-after was not counted"
    )
