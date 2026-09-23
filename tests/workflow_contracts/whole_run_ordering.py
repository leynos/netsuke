"""The rule a configured whole-run budget has to satisfy.

`.config/nextest.toml` sets one ``global-timeout``, so the contract over
it exercises this rule at a single point and would agree with every rule
that happens to accept that point. The rule therefore lives here, taking
its configuration and its lanes as parameters, and is driven with
configurations this repository does not have in
``whole_run_ordering_test``, including one omitting the key entirely.

Read by ``ty`` and run by pytest; runtime annotation introspection is not
supported here, because these annotations name ``TYPE_CHECKING``-only imports
and resolving one raises ``NameError``. ADR-038 records the decision.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

from nextest_budgets import (
    global_timeout,
    largest_test_allowance,
    termination_allowance,
)
from timeout_budgets import COLD_BUILD_ALLOWANCE_SECONDS, REPORT_PHASE_ALLOWANCE_SECONDS

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import fractions

    from coverage_lanes import CoverageLane


def watchdog_required_for(config_text: str) -> fractions.Fraction | None:
    """Return the watchdog a configured whole-run budget demands, or None.

    Four terms. The whole-run budget is what nextest may spend once tests
    begin; the termination allowance is what it may spend stopping them;
    the cold-build allowance is what ``cargo`` spends before nextest's
    clock starts at all, which the watchdog covers and the whole-run
    budget does not; and the report-phase allowance is what ``cargo
    llvm-cov`` spends after nextest's clock stops merging profile data and
    writing ``lcov.info``, which the watchdog likewise covers and the
    whole-run budget does not.

    Parameters
    ----------
    config_text : str
        The text of a nextest configuration file.

    Returns
    -------
    fractions.Fraction | None
        The watchdog those four terms require, exactly, or ``None`` when
        the configuration sets no ``global-timeout`` -- there is then no
        whole-run budget to derive a requirement from, and the ordering
        rule says nothing about a file that is incomplete rather than
        wrong.
    """
    whole_run = global_timeout(config_text)
    if whole_run is None:
        return None
    return (
        whole_run
        + termination_allowance(config_text)
        + COLD_BUILD_ALLOWANCE_SECONDS
        + REPORT_PHASE_ALLOWANCE_SECONDS
    )


def whole_run_ordering_faults(
    config_text: str, lanes: cabc.Iterable[CoverageLane]
) -> list[str]:
    """Return every way a configured whole-run budget breaks the ordering."""
    # An empty list is the only passing answer, and a configuration that
    # sets no `global-timeout` produces one: there is no tier three to
    # order. Faults are collected rather than raised one at a time so a
    # caller sees every lane at fault, not the first.
    whole_run = global_timeout(config_text)
    required = watchdog_required_for(config_text)
    if whole_run is None or required is None:
        return []
    faults = _per_test_faults(config_text, whole_run)
    faults.extend(_lane_faults(lanes, whole_run, required))
    return faults


def _per_test_faults(config_text: str, whole_run: fractions.Fraction) -> list[str]:
    """Return the fault, if any, in the whole run against one test."""
    largest = largest_test_allowance(config_text)
    if whole_run > largest:
        return []
    message = (
        f"the {whole_run:.0f}s global-timeout is not above the {largest:.0f}s "
        f"largest per-test allowance; the run would end before that test "
        f"could use its budget"
    )
    return [message]


def _lane_faults(
    lanes: cabc.Iterable[CoverageLane],
    whole_run: fractions.Fraction,
    required: fractions.Fraction,
) -> list[str]:
    """Return one fault per lane whose watchdog cannot cover the run."""
    faults = []
    for lane in lanes:
        if lane.watchdog is None:
            faults.append(f"{lane} sets no watchdog, so it inherits the default")
        elif lane.watchdog < required:
            faults.append(
                f"{lane} sets a {lane.watchdog:.0f}s watchdog, below the "
                f"{required:.0f}s needed to cover the {whole_run:.0f}s whole-run "
                f"budget, nextest's termination procedure, a cold build, and "
                f"the report phase that follows it; cargo would be killed "
                f"before nextest could report the overrun"
            )
    return faults
