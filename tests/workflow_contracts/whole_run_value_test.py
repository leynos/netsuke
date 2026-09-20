"""The whole-run budget's value, pinned to the figure the guide states.

The ordering assertions in ``timeout_ordering_test`` hold for every
budget between the 420 s largest per-test allowance and the 830 s the
1,800 s watchdog can cover, so the budget can drift to a value nobody
chose while each of them still passes. The guide records one value and
the arithmetic that produced it, and this is what makes changing the
file change the guide with it.

Split from ``timeout_ordering_test`` so neither module outgrows the
400-line limit the lint gate enforces.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
from nextest_budgets import global_timeout
from timeout_budgets import CAPPED_PROFILE, NEXTEST_CONFIG

#: The whole-run budget "Test timeouts: the tiers this repository sets"
#: in `docs/developers-guide.md` states, in seconds. Sized against the
#: longest measured nextest run and the watchdog above it; the guide
#: holds the sample and the arithmetic. It belongs to the profile CI
#: selects, not to `default`, which local runs use uncapped.
#:
#: Thirteen minutes rather than fifteen. The watchdog requirement the
#: ordering asserts grew a report-phase term, and at 900s the budget no
#: longer fitted inside the 1,800s watchdog it has to sit below.
STATED_WHOLE_RUN_BUDGET_SECONDS: typ.Final[float] = 13 * 60.0


def test_the_whole_run_budget_is_the_value_the_guide_states() -> None:
    """Tier two is pinned by value, not only by its place in the order.

    Proved by mutation: changing ``global-timeout`` to any other
    duration fails this test, including values the ordering assertions
    accept.
    """
    configured = global_timeout(NEXTEST_CONFIG.read_text(encoding="utf-8"))

    assert configured == pytest.approx(STATED_WHOLE_RUN_BUDGET_SECONDS), (
        f"[profile.{CAPPED_PROFILE}] sets a global-timeout of {configured}s "
        f"where the developers' guide states "
        f"{STATED_WHOLE_RUN_BUDGET_SECONDS:.0f}s; the "
        f"ordering assertions accept a wide range, so a value nobody chose "
        f"passes them all, and the guide holds the sample it was sized from"
    )
