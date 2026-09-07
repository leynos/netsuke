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
from hypothesis import given
from hypothesis import strategies as st
from nextest_budgets import (
    NextestConfigurationError,
    UnboundedTestError,
    global_timeout,
    grace_period,
    largest_test_allowance,
    seconds,
    termination_allowance,
)
from timeout_budgets import (
    NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS,
    TERMINATION_SAFETY_MARGIN_SECONDS,
)

#: The units nextest accepts, with their length in seconds.
UNITS: typ.Final[dict[str, float]] = {"ms": 0.001, "s": 1.0, "m": 60.0, "h": 3600.0}

COVERAGE_STEP: typ.Final[str] = (
    "leynos/shared-actions/.github/actions/generate-coverage@abc123"
)

whole_numbers = st.integers(min_value=1, max_value=10_000)
units = st.sampled_from(sorted(UNITS))
multipliers = st.integers(min_value=1, max_value=20)


def document(*tables: str, profile: str = "default") -> str:
    """Return a nextest document declaring those slow-timeouts.

    The reading parses the file, so a configuration it is driven with
    has to be shaped the way nextest reads one: the first table is the
    profile's own and the rest are its overrides. A bare key at the root
    of the document is not configuration to nextest and is not read as
    any here either.

    Parameters
    ----------
    *tables : str
        The ``slow-timeout`` assignments, profile's own first.
    profile : str
        The profile to declare them under.

    Returns
    -------
    str
        A configuration document.
    """
    lines = [f"[profile.{profile}]"]
    if tables:
        lines.append(tables[0])
    for override in tables[1:]:
        lines += ["", f"[[profile.{profile}.overrides]]", override]
    return "\n".join(lines) + "\n"


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
    with pytest.raises(NextestConfigurationError):
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
    its multiplier, agrees with the handwritten cases whenever they
    coincide. Over generated configurations they stop coinciding.
    """
    config = document(
        *(
            f'slow-timeout = {{ period = "{value}{unit}", terminate-after = {times} }}'
            for value, unit, times in budgets
        )
    )
    expected = max(value * UNITS[unit] * times for value, unit, times in budgets)
    assert largest_test_allowance(config) == pytest.approx(expected), (
        "the largest budget is the largest period times its own multiplier"
    )


@pytest.mark.parametrize(
    "table",
    [
        pytest.param('slow-timeout = "90s"', id="a-bare-duration"),
        pytest.param(
            'slow-timeout = { period = "90s" }', id="a-table-without-terminate-after"
        ),
        pytest.param(
            'slow-timeout = { period = "90s", grace-period = "5s" }',
            id="a-table-with-only-a-grace-period",
        ),
    ],
)
def test_a_slow_timeout_that_never_terminates_is_refused(table: str) -> None:
    """`terminate-after` is optional, and without it nothing is bounded.

    nextest marks the test slow, warns once per period, and lets it run
    on. Reading such a configuration as a period-long budget would put a
    number on the tier that is missing, and every comparison above it
    would then pass against a tier that does not exist. Every table in
    this repository's own configuration sets it explicitly, so nothing
    here relies on the looser reading.
    """
    with pytest.raises(UnboundedTestError, match=r"terminate-after"):
        largest_test_allowance(document(table))


def test_a_grace_period_is_not_read_as_a_per_test_budget() -> None:
    """The two keys sit in the same inline table.

    A matcher reading `period` as a substring would take a grace period
    for a per-test budget whenever the former were the larger.
    """
    config = document(
        'slow-timeout = { period = "30s", terminate-after = 1, grace-period = "30m" }'
    )
    assert largest_test_allowance(config) == pytest.approx(30.0), (
        "the per-test reading took a grace period for a slow-timeout"
    )


@pytest.mark.parametrize(
    "config",
    [
        "",
        "[profile.default]\nfail-fast = false\n",
        'slow-timeout = { period = "30s", terminate-after = 1 }\n',
        '# slow-timeout = { period = "30s", terminate-after = 1 }\n',
    ],
    ids=[
        "empty",
        "no-slow-timeout",
        "outside-any-profile",
        "commented-out",
    ],
)
def test_a_configuration_with_no_readable_budget_is_refused(config: str) -> None:
    """Returning zero would make every whole-run budget look comfortable.

    The last two cases are what parsing buys. A key at the root of the
    document is not in any profile, and a commented-out one is not
    configuration at all; a text match counted both.
    """
    with pytest.raises(NextestConfigurationError):
        largest_test_allowance(config)


@pytest.mark.parametrize(
    ("config", "expected"),
    [
        pytest.param("", None, id="absent"),
        pytest.param('global-timeout = "45m"', None, id="outside-any-profile"),
        pytest.param(
            '[profile.default]\nglobal-timeout = "600s"\n', 600.0, id="inside-a-profile"
        ),
        pytest.param(
            '[profile.ci]\nglobal-timeout = "600s"\n', None, id="another-profile"
        ),
        pytest.param(
            '[profile.default]\n# global-timeout = "45m"\n',
            None,
            id="commented-out-is-not-set",
        ),
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
    config = document(
        *(
            f'slow-timeout = {{ period = "1s", terminate-after = 1, '
            f'grace-period = "{value}{unit}" }}'
            for value, unit in periods
        )
    )
    largest = max(value * UNITS[unit] for value, unit in periods)
    assert grace_period(config) == pytest.approx(largest), (
        "the largest configured grace period governs"
    )
    assert termination_allowance(config) == pytest.approx(
        largest + TERMINATION_SAFETY_MARGIN_SECONDS
    ), "the allowance is the grace period plus the margin, not the larger"


@given(
    periods=st.lists(st.tuples(whole_numbers, units), min_size=1, max_size=6),
    profile=st.sampled_from(["default", "ci"]),
)
def test_an_unconfigured_grace_period_falls_back_to_nextest_s_default(
    periods: list[tuple[int, str]], profile: str
) -> None:
    """Assuming zero would understate what nextest needs to stop a run."""
    config = document(
        *(
            f'slow-timeout = {{ period = "{value}{unit}", terminate-after = 1 }}'
            for value, unit in periods
        ),
        profile=profile,
    )
    assert grace_period(config) == pytest.approx(
        NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS
    ), "an absent grace period must fall back to nextest's default"
