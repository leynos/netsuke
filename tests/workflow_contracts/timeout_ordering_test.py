"""Contract for the timers that can end a test run.

Four independent budgets can end a coverage lane, each set somewhere
different, and they only work if each sits above the one inside it. All
four are set here: a per-test ``slow-timeout`` and a whole-run
``global-timeout`` in ``.config/nextest.toml``, the shared coverage
action's wall-clock watchdog on the ``cargo`` invocation, and the job's
own ``timeout-minutes``.

The whole-run tier was the last to arrive, and its absence is the state
this contract exists to keep from returning. Without it the watchdog
does tier two's job as well as its own: a run whose tests each stay
inside their allowance can still exceed the watchdog between them, and
the failure then names ``cargo`` rather than the run. The contract
therefore asserts that the budget is present as well as where it sits,
because a commented-out key is what a presence assertion catches and an
ordering assertion does not.

The per-test allowance is ``period`` multiplied by ``terminate-after``,
not ``period`` alone. Reading the period as the budget would understate
the largest allowance here fivefold on Linux and sevenfold on Windows.

See "Test timeouts: the tiers this repository sets" in
``docs/developers-guide.md``, and the canonical wording in
`leynos/shared-actions`' `generate-coverage` README.

Run via ``make test-workflow-contracts``.
"""

import re
import typing as typ

import pytest
from coverage_lanes import (
    CoverageLane,
    conditions_by_coordinate,
    coverage_lanes_of,
)
from lane_environment import watchdog_of
from nextest_budgets import (
    global_timeout,
    largest_test_allowance,
    termination_allowance,
)
from nextest_durations import (
    seconds,
)
from timeout_budgets import (
    CAPPED_PROFILE,
    CEILING_MARGIN_SECONDS,
    COVERAGE_ACTION,
    NEXTEST_CONFIG,
    NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS,
    OUTSIDE_WATCHDOG_ALLOWANCE_SECONDS,
    TERMINATION_SAFETY_MARGIN_SECONDS,
    WATCHDOG_VARIABLE,
    required_ceiling,
)
from whole_run_ordering import whole_run_ordering_faults

#: The condition each coverage lane legitimately carries, keyed by
#: workflow and job, as the step's ``if`` and its job's.
#:
#: A skipped step runs no `cargo`, so its watchdog never arms and every
#: assertion below says nothing about it. `if: false` on either would
#: leave a lane that looks bounded and is not. The values are pinned
#: rather than merely tolerated, because a lane gaining, losing or
#: changing a condition changes when it runs at all.
#:
#: `ci.yml` also runs on pushes, where the trunk lane covers the same
#: ground, so its coverage step is conditional on the pull request.
#: Keyed by workflow, job and step, because a job may run the coverage
#: action twice and the steps need not carry the same condition; keying
#: by job alone let the second overwrite the first. The entry pins a step
#: rather than a lane: a step passing `doctests: 'true'` arms two
#: watchdog windows, and both of them carry this one condition.
REQUIRED_CONDITIONS: typ.Final[dict[tuple[str, str, str], tuple[object, object]]] = {
    ("ci.yml", "build-test", "Test and Measure Coverage"): (
        "github.event_name == 'pull_request'",
        None,
    ),
    ("coverage-main.yml", "coverage-upload", "Test and Measure Coverage"): (
        None,
        None,
    ),
}


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


def _budgets_per_job(
    coverage_lanes: tuple[CoverageLane, ...],
) -> dict[tuple[str, str], list[CoverageLane]]:
    """Group the lanes by the job whose ceiling has to contain them.

    A lane is one coverage step. The ceiling belongs to the job, so a
    job running the action twice must contain both budgets, and judging
    each lane separately against the same ceiling asks for the larger
    of the two rather than their sum.

    Parameters
    ----------
    coverage_lanes : tuple[CoverageLane, ...]
        Every coverage step in the tree.

    Returns
    -------
    dict
        The lanes of each job, keyed by workflow and job identifier.
    """
    grouped: dict[tuple[str, str], list[CoverageLane]] = {}
    for lane in coverage_lanes:
        grouped.setdefault((lane.workflow, lane.job), []).append(lane)
    return grouped


def test_the_job_ceiling_covers_every_watchdog_and_the_work_around_them(
    coverage_lanes: tuple[CoverageLane, ...],
) -> None:
    """Tier four must not pre-empt tier three, for the job as a whole.

    The two clocks do not start together. The job timer starts when the
    job starts, before the checkout, the toolchain setup and the linting
    that precede coverage, and it is still running through whatever
    follows. The watchdog starts when `cargo` does. A ceiling merely
    above the watchdog still cancels the job before the watchdog can
    report an overrun, and a cancellation discards the log that would
    have explained it.

    The ceiling belongs to the job rather than to a step, so the lanes
    are summed per job before the comparison. Judging each lane
    separately against the same ceiling asks only that it clear the
    largest of them, which is the requirement a job running the action
    once happens to satisfy and a job running it twice does not.
    """
    for (workflow, job), lanes in _budgets_per_job(coverage_lanes).items():
        budgets = [lane.watchdog for lane in lanes if lane.watchdog is not None]
        assert len(budgets) == len(lanes), f"{workflow}:{job} has an unset watchdog"
        ceiling = lanes[0].job_timeout
        assert ceiling is not None, (
            f"{workflow}:{job} runs {len(budgets)} watchdog-bounded cargo "
            f"invocation(s) in a job with no timeout-minutes; the outermost "
            f"tier is missing and GitHub's six-hour default applies"
        )
        required = required_ceiling(budgets)
        assert ceiling >= required, (
            f"{workflow}:{job} has a job ceiling of {ceiling:.0f}s, below the "
            f"{required:.0f}s needed to cover {len(budgets)} watchdog(s) "
            f"totalling {sum(budgets):.0f}s, "
            f"{OUTSIDE_WATCHDOG_ALLOWANCE_SECONDS:.0f}s of measured work "
            f"outside them, and a {CEILING_MARGIN_SECONDS:.0f}s margin above "
            f"that sum; an overrun would be cancelled rather than reported"
        )


def test_a_whole_run_budget_sits_inside_the_watchdog(
    coverage_lanes: tuple[CoverageLane, ...], nextest_config: str
) -> None:
    """Tier three must not pre-empt tier two, and tier two must exist.

    The presence assertion comes first and is not a formality. Every
    ordering assertion below reads a budget from the file, so commenting
    the key out or deleting it satisfies each of them by leaving nothing
    to compare, and the watchdog silently resumes doing tier two's job.

    The ordering rule itself is ``whole_run_ordering_faults``, driven
    with configurations this repository does not have in
    ``whole_run_ordering_test``, because a rule exercised only against
    one file is a rule with one input.

    Proved by mutation: commenting out the ``global-timeout``, lowering
    it below the largest per-test allowance, and raising it above what
    the watchdog can cover each fail this test.
    """
    assert global_timeout(nextest_config) is not None, (
        f"[profile.{CAPPED_PROFILE}] sets no global-timeout, so nothing "
        f"bounds the test run as a whole and the cargo watchdog is doing tier "
        f"two's job; a run whose tests each stay inside their allowance can "
        f"exceed the watchdog between them, and the failure then names cargo "
        f"rather than the run"
    )
    faults = whole_run_ordering_faults(nextest_config, coverage_lanes)
    assert not faults, "; ".join(faults)


def test_the_largest_per_test_allowance_counts_the_multiplier(
    nextest_config: str,
) -> None:
    """``terminate-after`` scales the period; the budget is their product.

    This is the reading that decides every comparison above, and it is
    the one easy to get wrong: the periods here are all 60 s, so a
    contract reading the period alone would report a 60 s largest
    allowance where the real figure is 420 s.
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


def test_the_termination_allowance_is_the_grace_period_plus_the_margin() -> None:
    """The two terms are added, not maximized over.

    A single floor over the grace period and the margin would absorb
    every grace period below the margin, so adding one of thirty seconds
    to this configuration would demand nothing more of the watchdog above
    it. The ordering assertion that uses this reading has 50 s of slack
    against the real tree -- the 1,800 s watchdog against a 1,750 s
    requirement -- so it would pass on either reading; the test of the
    reading itself is what stands behind the two terms.
    """
    assert termination_allowance("") == pytest.approx(
        NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS + TERMINATION_SAFETY_MARGIN_SECONDS
    ), "an unnamed grace period must fall back to nextest's own default"
    configured = termination_allowance(
        "[profile.default]\n"
        'slow-timeout = { period = "60s", terminate-after = 1, '
        'grace-period = "30s" }\n'
    )
    assert configured == pytest.approx(30.0 + TERMINATION_SAFETY_MARGIN_SECONDS), (
        "a grace period below the margin must still raise the allowance; "
        "a maximum over the two terms would have discarded it"
    )
    largest = termination_allowance(
        "[profile.default]\n"
        'slow-timeout = { period = "60s", terminate-after = 1, '
        'grace-period = "5s" }\n'
        "\n[[profile.default.overrides]]\n"
        'slow-timeout = { period = "60s", terminate-after = 1, '
        'grace-period = "45s" }\n'
    )
    assert largest == pytest.approx(45.0 + TERMINATION_SAFETY_MARGIN_SECONDS), (
        "the largest configured grace period governs the allowance"
    )


def test_the_watchdog_is_resolved_from_every_environment_scope() -> None:
    """Step, then job, then workflow, as GitHub resolves them.

    Both workflows here set the value at job level, so a reading that
    consulted only the step would report every lane as inheriting the
    action's undocumented default and the contract would fail loudly.
    A reading that stopped at the job would pass on this tree while
    missing a workflow-level value entirely, which is the case this
    covers.
    """
    document = {"env": {WATCHDOG_VARIABLE: "1200"}}
    job = {"env": {WATCHDOG_VARIABLE: "1800"}}
    step = {"env": {WATCHDOG_VARIABLE: "2400"}}
    assert watchdog_of(document, job, step) == pytest.approx(2400.0), (
        "a step's own value wins over the job's and the workflow's"
    )
    assert watchdog_of(document, job, {}) == pytest.approx(1800.0), (
        "the job's value applies when the step names none"
    )
    assert watchdog_of(document, {}, {}) == pytest.approx(1200.0), (
        "the workflow's value applies when neither the step nor the job "
        "names one; a reading that stopped at the job would return None "
        "and report the lane as inheriting the action's default"
    )
    assert watchdog_of({}, {}, {}) is None, (
        "no level naming the variable must read as absent, not as a number"
    )


def test_each_coverage_lane_carries_the_condition_it_is_meant_to(
    coverage_lanes: tuple[CoverageLane, ...],
) -> None:
    """A skipped step runs no `cargo`, so its watchdog never arms.

    Every assertion above reads a lane's declared budgets and says
    nothing about whether the step runs. `if: false` on the step or its
    job would leave a lane that looks bounded and is not, and so would a
    plausible condition quietly excluding the event the lane exists for.

    The conditions are pinned rather than forbidden, because the one
    here is legitimate: `ci.yml` also runs on pushes, which the trunk
    lane covers. Pinning it means a lane gaining, losing or changing a
    condition has to change this contract and the guide with it. The
    coordinates are compared both ways first, so a new lane with no
    entry here fails rather than passing unexamined, and a lane that
    disappeared fails rather than being skipped.

    Proved by mutation: `if: false` on the coverage step, the same on
    its job, a push-only condition, and a coordinate dropped from
    ``REQUIRED_CONDITIONS`` each fail this test.

    The coordinate carries the step's name as well as its job, since a
    job may run the coverage action twice and the two steps need not
    carry the same condition; keyed by job alone, the second overwrote
    the first. An unnamed step takes its job's name, so a coordinate can
    hold two lanes from two steps, and each therefore carries the
    sequence found there rather than one condition. A pinned entry is a
    step rather than a lane, so a step arming two watchdog windows
    appears twice at its coordinate and both appearances are held to the
    condition pinned here.
    """
    found = conditions_by_coordinate(coverage_lanes)
    assert set(found) == set(REQUIRED_CONDITIONS), (
        f"the coverage lanes are not the ones this contract pins: "
        f"unlisted {sorted(set(found) - set(REQUIRED_CONDITIONS))}, missing "
        f"{sorted(set(REQUIRED_CONDITIONS) - set(found))}; a lane with no "
        f"entry here is a lane whose condition nobody has judged"
    )
    wrong = {
        coordinate: (expected, found[coordinate])
        for coordinate, expected in REQUIRED_CONDITIONS.items()
        if any(condition != expected for condition in found[coordinate])
    }
    assert not wrong, (
        f"these coverage lanes do not carry the conditions the developers' "
        f"guide records, as expected versus found: {wrong}; a lane that is "
        f"skipped runs no cargo, so its watchdog never arms. Every lane at a "
        f"coordinate is held to the pinned condition, because the windows one "
        f"step arms are that step's windows and a step is skipped whole"
    )
