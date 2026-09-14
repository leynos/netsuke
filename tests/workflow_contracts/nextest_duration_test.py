"""What the duration reader accepts, and what it refuses.

Split from ``timeout_budget_properties_test`` so neither module
outgrows the 400-line limit the lint gate enforces, and because the
grammar is a separable claim: every ordering the timeout contract
asserts is an inequality between two numbers this reader produced, so a
wrong unit or a wrongly accepted spelling turns the whole contract into
a comparison of plausible wrong values.

The values below were measured against humantime 2.3.0, the version the
lockfile of the pinned cargo-nextest release resolves, by running its
parser rather than by reading about it.
"""

import typing as typ

import pytest
from hypothesis import given
from hypothesis import strategies as st
from nextest_durations import NextestConfigurationError, seconds

#: The four units the repository's own configuration uses, with their
#: lengths, for the scaling property below.
UNITS: typ.Final[dict[str, float]] = {"ms": 0.001, "s": 1.0, "m": 60.0, "h": 3600.0}

whole_numbers = st.integers(min_value=1, max_value=10_000)
units = st.sampled_from(sorted(UNITS))


@pytest.mark.parametrize(
    ("duration", "expected"),
    [
        pytest.param("500ms", 0.5, id="milliseconds"),
        pytest.param("90s", 90.0, id="seconds"),
        pytest.param("15m", 900.0, id="minutes"),
        pytest.param("2h", 7200.0, id="hours"),
        pytest.param("  45s  ", 45.0, id="surrounding-whitespace"),
        pytest.param("2h 30m", 9000.0, id="a-composite-with-a-space"),
        pytest.param("1m30s", 90.0, id="a-composite-without-a-space"),
        pytest.param("1d", 86400.0, id="days"),
        pytest.param("15sec", 15.0, id="a-long-unit-spelling"),
        pytest.param("1.5m", 90.0, id="a-fractional-value"),
        pytest.param("0.5s", 0.5, id="a-fraction-below-one"),
        pytest.param("1 . 5 m", 90.0, id="a-fraction-spaced-around-the-point"),
        pytest.param("1wk", 604800.0, id="the-abbreviated-week"),
        pytest.param("2wks", 1209600.0, id="the-abbreviated-plural-week"),
        pytest.param("1yr", 31557600.0, id="the-abbreviated-year"),
        pytest.param("3yrs", 94672800.0, id="the-abbreviated-plural-year"),
        pytest.param("1 0s", 10.0, id="whitespace-inside-the-number"),
        pytest.param("0", 0.0, id="a-bare-zero-with-no-unit"),
        pytest.param("500nanos", 5e-7, id="the-long-nanosecond-spelling"),
        pytest.param("250millis", 0.25, id="the-long-millisecond-spelling"),
        pytest.param("750\u00b5s", 0.00075, id="the-micro-sign"),
        pytest.param("1.5h", 5400.0, id="a-fraction-of-an-hour-in-whole-seconds"),
        pytest.param("0.123s", 0.123, id="a-fraction-of-a-second-in-nanoseconds"),
        pytest.param("0.000000001m", 6e-8, id="a-fraction-of-a-minute-in-nanoseconds"),
    ],
)
def test_each_unit_converts_exactly(duration: str, expected: float) -> None:
    """The unit table decides every comparison the contract makes.

    A single wrong entry would leave every downstream assertion an
    inequality between two plausible numbers, so each unit is pinned
    rather than sampled.

    The fractional values, the abbreviated week and year, the number
    carrying whitespace and the bare zero were measured against
    humantime 2.3.0, the version the pinned cargo-nextest release locks,
    rather than assumed: this reader had refused all of them, which is
    the fault the module exists to avoid. Its parser ignores whitespace
    while it accumulates a number, so `1 0s` is ten seconds, and reads a
    bare `0` as a zero duration needing no unit.

    `nanos` and `millis` come from humantime's own unit table, which
    takes three spellings each where this reader had two; the micro
    sign is its one non-ASCII spelling, U+00B5 not U+03BC.
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
    [
        "",
        "300",
        "s",
        "five minutes",
        "-30s",
        ".5s",
        "1.s",
        "1.5.5m",
        "1_000s",
        "30 fortnights",
        "1.5ns",
        "0.5ns",
        "0.123h",
        "0.0000000001s",
        " 0 ",
        "00",
    ],
    ids=[
        "empty",
        "no-unit",
        "no-value",
        "words",
        "negative",
        "a-fraction-with-no-whole-part",
        "a-point-with-no-fraction-after-it",
        "two-points",
        "a-digit-separator",
        "a-unit-humantime-does-not-know",
        "a-fractional-nanosecond",
        "half-a-nanosecond",
        "a-fraction-of-an-hour-below-a-second",
        "a-fraction-below-a-nanosecond",
        "a-padded-bare-zero",
        "a-repeated-bare-zero",
    ],
)
def test_an_unreadable_duration_is_refused(duration: str) -> None:
    """A duration nextest would reject must not become a number.

    Returning something plausible would put a comparison against a
    budget nextest never applies, and the contract would pass while the
    ordering it claims to hold did not. Each of these was checked
    against humantime 2.3.0 and refused there: a fraction needs a whole
    part before the point and a digit after it, values are unsigned, and
    the only separators are whitespace. `0` is the one value that may
    carry no unit, so `300` stays refused, and the special case is the
    exact text, so `" 0 "` and `"00"` are not it.

    The four fractions are the case a float reader gets wrong in the
    other direction. humantime carries a fraction as a numerator over a
    power of ten and divides with a remainder check, so it has no step
    below a nanosecond and refuses a fractional one outright; and for
    hours and longer it divides whole seconds, which is why `0.123h` is
    refused where `0.123s` is exact. Read as floats these four become
    5e-10, 1.5e-09, 442.8 and 1e-10, none of which nextest would have
    started with.
    """
    with pytest.raises(NextestConfigurationError):
        seconds(duration)
