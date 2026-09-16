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

import re
import typing as typ

import pytest
from hypothesis import given
from hypothesis import strategies as st
from nextest_durations import (
    _DIGIT_CHARS,
    _SPACE_CHARS,
    NextestConfigurationError,
    _digits,
    seconds,
)

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


@pytest.mark.parametrize(
    ("duration", "float_reading"),
    [
        pytest.param("1\x1cs", 1.0, id="a-file-separator-before-the-unit"),
        pytest.param("\x1c45m", 2700.0, id="a-file-separator-leading"),
        pytest.param("45m\x1f", 2700.0, id="a-unit-separator-trailing"),
        pytest.param("1\x1d0s", 10.0, id="a-group-separator-inside-the-number"),
    ],
)
def test_an_information_separator_is_refused(
    duration: str, float_reading: float
) -> None:
    r"""The four C0 separators are not whitespace to humantime.

    Rust's ``char::is_whitespace`` is the Unicode White_Space property
    and Python's ``\s`` is that property plus U+001C to U+001F, the
    file, group, record and unit separators. Scanning every code point
    finds those four and nothing else disagreeing in either direction.
    A reader spelling its class ``\s``, or trimming with
    ``str.strip``, therefore skips them wherever it skips a space, and
    each of these reads as the number named beside it from a
    configuration nextest refuses at startup.

    The separator between two digits is a case of its own because it is
    the shape nobody would notice in a file: ``1\x1d0s`` looks like
    ``1 0s``, which is ten seconds, and is not.
    """
    assert float_reading, "each case names the number the defect produced"
    with pytest.raises(NextestConfigurationError):
        seconds(duration)


@pytest.mark.parametrize(
    "duration",
    [
        pytest.param("\u0665s", id="an-arabic-indic-numeral-alone"),
        pytest.param("\u096ams", id="a-devanagari-numeral-alone"),
        pytest.param("1\u0660s", id="an-arabic-indic-numeral-inside-a-number"),
        pytest.param("1.\u0665s", id="an-arabic-indic-numeral-in-a-fraction"),
    ],
)
def test_a_non_ascii_digit_is_refused(duration: str) -> None:
    r"""ASCII digits are what humantime reads, and ``\d`` is wider.

    ``\d`` matches every Unicode decimal digit, so a reader spelling
    its digit class that way converts numerals nextest refuses at
    startup: these four read as five seconds, four milliseconds, ten
    seconds and one and a half seconds. The mirror image of the
    separators above, and ``1\u0660s`` is the one that matters most,
    being an ordinary-looking number with one character wrong.
    """
    with pytest.raises(NextestConfigurationError):
        seconds(duration)


def test_the_whitespace_class_is_rusts_own() -> None:
    r"""Pin the reader's whitespace class against Python's, both ways.

    The refusals above name four inputs. This names the rule they come
    from, over the first 0x11000 code points, which covers every plane
    with a White_Space character in it. Python's ``\s`` exceeds this
    reader's class by exactly U+001C to U+001F, and the reader's class
    exceeds Python's by nothing. Written in both directions because a
    one-directional assertion passes on a class that has merely grown:
    what fails here is either language's notion of whitespace moving,
    rather than a runner.
    """
    ours = set(_SPACE_CHARS)
    pythons = {
        chr(point) for point in range(0x11000) if re.fullmatch(r"\s", chr(point))
    }
    assert pythons - ours == {"\x1c", "\x1d", "\x1e", "\x1f"}, (
        "Python's whitespace should exceed humantime's by exactly the four "
        "information separators"
    )
    assert not ours - pythons, (
        "every character this reader skips should be one Python calls "
        "whitespace, or the class has grown a character Rust does not skip"
    )


def test_the_digit_class_is_humantimes_own() -> None:
    r"""Pin the reader's digit class against Python's, both ways.

    The companion to the whitespace contract. humantime reads
    ``'0'..='9'`` and nothing else, so the reader's class must be
    exactly those ten, and Python's ``\d`` must exceed it rather than
    coincide with it: if it ever stopped exceeding it, the reason for
    spelling the class out would have gone and this contract should be
    reconsidered rather than silently kept.
    """
    ours = set(_DIGIT_CHARS)
    ascii_digits = {chr(point) for point in range(ord("0"), ord("9") + 1)}
    pythons = {
        chr(point) for point in range(0x11000) if re.fullmatch(r"\d", chr(point))
    }
    assert ours == ascii_digits, (
        "the reader's digit class should be exactly '0' to '9', which is "
        "what humantime matches"
    )
    assert pythons - ours, (
        "Python's digit class should still exceed ASCII, which is why the "
        "reader writes the ten out"
    )
    assert all(seconds(f"1{digit}s") for digit in sorted(ours - {"0"})), (
        "every digit in the class should still read as a digit"
    )


def test_a_digit_run_keeps_a_separator_it_was_never_given() -> None:
    """``_digits`` removes only what the pattern tolerated.

    Unreachable through ``seconds``: the pattern refuses a separator
    inside a number, so this helper never sees one. It is exercised
    directly because ``str.split``, which it used to use, would drop
    U+001C to U+001F silently, and a later widening of the pattern
    would then turn a refusal into a different number rather than into
    a failure. Without this the site would survive a mutation back to
    ``str.split`` and prove nothing.
    """
    assert _digits("1 0") == "10", "tolerated whitespace should go"
    assert _digits("1\x1d0") == "1\x1d0", "a separator should not"


@pytest.mark.parametrize(
    "duration",
    [
        pytest.param("18446744073709551616s", id="one-second-past-the-ceiling"),
        pytest.param("307445734561825861m", id="a-minute-product-that-overflows"),
        pytest.param("584542046091y", id="a-year-product-that-overflows"),
        pytest.param("99999999999999999999999999s", id="far-past-the-ceiling"),
        pytest.param("18446744073709551616ns", id="a-nanosecond-count-past-it"),
        pytest.param(
            "18446744073709551615ns 18446744073709551615ns",
            id="a-nanosecond-sum-that-overflows-well-inside-the-range",
        ),
        pytest.param("18446744073709551615s 1s", id="a-sum-of-seconds"),
        pytest.param(
            "18446744073709551615s 1000000000ns", id="a-carry-from-nanoseconds"
        ),
        pytest.param("18446744073709551614s 2000000000ns", id="a-carry-of-two"),
        pytest.param("18446744073709551615s 500ms 500ms", id="a-carry-of-halves"),
        pytest.param("18446744073709551615s 1000ms", id="a-carry-from-millis"),
        pytest.param("18446744073709551615.5us", id="a-fraction-numerator-past-it"),
        pytest.param("18446744073709551.5ms", id="a-scaled-fraction-past-it"),
        pytest.param("1.00000000000000000000s", id="a-denominator-past-it"),
    ],
)
def test_a_duration_outside_humantimes_range_is_refused(duration: str) -> None:
    """Durations past 64 bits are refused, as humantime refuses them.

    Each of these was refused by humantime 2.3.0 and read as a number
    by this reader before the range model landed. They are not one
    check: humantime keeps whole seconds and a nanosecond part, both
    ``u64``, and checks every product and every sum, so a value can be
    refused for overflowing the nanosecond part while its total sits
    comfortably inside the seconds range. The two nanosecond maxima
    above name about 1,169 years between them and are refused for
    exactly that reason.
    """
    with pytest.raises(NextestConfigurationError):
        seconds(duration)


@pytest.mark.parametrize(
    ("duration", "expected"),
    [
        pytest.param("18446744073709551615s", 18446744073709551615.0, id="the-ceiling"),
        pytest.param("0.5s 0.5s", 1.0, id="a-carry-that-fits"),
        pytest.param("1000000000ns", 1.0, id="a-nanosecond-count-that-carries"),
        pytest.param("0s 1000000000ns", 1.0, id="a-carry-onto-a-zero"),
        pytest.param("1.0000000000000000000s", 1.0, id="a-denominator-that-fits"),
        pytest.param("307445734561825860m", 18446744073709551600.0, id="minutes-at-it"),
        pytest.param("584542046090y", 18446744073689784000.0, id="years-near-it"),
        pytest.param(
            "18446744073709551615ns 1ns",
            18446744073.709551616,
            id="a-nanosecond-maximum-then-one-more",
        ),
        pytest.param(
            "18446744073709551614s 1000000000ns 1ns",
            18446744073709551615.000000001,
            id="three-parts-where-the-middle-one-carries",
        ),
    ],
)
def test_a_duration_at_humantimes_limit_is_read(duration: str, expected: float) -> None:
    """The range model must not be merely stricter.

    One mechanism gives two answers here. humantime declines to
    normalize a nanosecond part of exactly one second, leaving it to
    the conversion that follows, which carries it and aborts when the
    carry overflows. So ``0.5s 0.5s`` is one second while
    ``18446744073709551615s 500ms 500ms`` is refused, and a reader made
    strict enough to refuse the second by refusing every carry fails
    the first. Both are here for that reason.

    The last two pin the order the parts are summed in.
    ``18446744073709551615ns 1ns`` is read only because the first part
    is carried into seconds before the second arrives: taken the other
    way round the nanosecond accumulator overflows and the whole
    duration is refused, which is a refusal of a configuration nextest
    would have run. It is the one input of the seventy-one that tells
    the two orders apart.
    """
    assert seconds(duration) == pytest.approx(expected), (
        f"{duration!r} is inside humantime's range and must still be read"
    )
