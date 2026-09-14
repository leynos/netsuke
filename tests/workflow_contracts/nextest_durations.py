"""Nextest durations, and the faults a configuration can carry.

Split from ``nextest_budgets`` so both stay inside the 400-line limit
the lint gate enforces. nextest parses durations with `humantime`
through `humantime_serde`, which reads a sequence of value-and-unit
pairs and sums them, so `2h 30m` and `1d` are valid and a parser taking
one pair would refuse configuration the runner accepts.

The grammar was measured against humantime 2.3.0, the version the
lockfile of the pinned cargo-nextest release resolves, rather than
assumed. A value may carry a fractional part with whitespace tolerated
around the point, so `1.5m` and `1 . 5 m` are both ninety seconds, and
`wk`, `wks`, `yr` and `yrs` are accepted alongside the longer spellings.
Whitespace inside the number is ignored as well, so `1 0s` is ten
seconds, and a bare `0` is a zero duration needing no unit at all.

Fractions are exact integer arithmetic there, not floating point, and
the arithmetic differs by unit: see :class:`_Unit`. Reading them as
floats accepted four shapes humantime refuses, measured against
2.3.0: `0.5ns`, `1.5ns`, `0.123h` and `0.0000000001s`. Accepting what
nextest refuses is the same fault as refusing what it accepts, seen
from the other side: the contract would assert an ordering over a
budget the runner would never have started with.
"""

import re
import typing as typ

#: Digits with whitespace tolerated between them. humantime's parser
#: ignores whitespace while it accumulates a number, so `1 0s` is ten
#: seconds rather than a malformed duration.
_SPACED_DIGITS: typ.Final[str] = r"\d(?:\s*\d)*"

#: One value-and-unit pair. The fractional part is optional and
#: humantime tolerates whitespace around the point; a leading point, a
#: trailing point, a second point, a sign and a digit separator are all
#: refused there and so are refused here. The whole part and the
#: fraction are captured apart because humantime scales them
#: differently.
_DURATION_TOKEN: typ.Final[re.Pattern[str]] = re.compile(
    rf"(?P<whole>{_SPACED_DIGITS})"
    rf"(?:\s*\.\s*(?P<fraction>{_SPACED_DIGITS}))?"
    r"\s*(?P<unit>[A-Za-z\u00b5]+)\s*"
)

#: The one duration humantime accepts with no unit. Its parser
#: special-cases the exact text before reading a single character, so
#: the comparison here is against the raw value rather than a stripped
#: one: ``" 0 "`` is not this case and nextest refuses it.
_BARE_ZERO: typ.Final[str] = "0"

#: Nanoseconds in a second, which is the scale this port works in.
_SECOND: typ.Final[int] = 1_000_000_000


class _Unit(typ.NamedTuple):
    """One humantime unit, as its parser treats it.

    Attributes
    ----------
    nanoseconds : int
        One of this unit in nanoseconds. A value's whole part is
        multiplied by this.
    fraction_scale : int or None
        What a fraction's numerator is multiplied by before the exact
        division humantime requires, or None when the unit admits no
        fraction at all. ``ns`` is that case: humantime refuses a
        fractional nanosecond outright rather than rounding it.
    fraction_in_seconds : bool
        Whether that division yields seconds rather than nanoseconds.
        humantime divides whole seconds for hours and longer, so
        ``0.123h`` is refused where ``0.123s`` is exact. A reader
        working in nanoseconds throughout would accept durations
        nextest rejects, and one working in floats accepts every
        inexact fraction at every unit.
    """

    nanoseconds: int
    fraction_scale: int | None
    fraction_in_seconds: bool


#: Every spelling `humantime` accepts, grouped by the unit it names,
#: with humantime's own definitions of a month and a year. Spelt out in
#: full rather than trimmed to the plausible ones, because refusing a
#: unit nextest accepts would fail a configuration the runner is happy
#: with.
_UNIT_SPELLINGS: typ.Final[tuple[tuple[tuple[str, ...], _Unit], ...]] = (
    (
        ("nanos", "nsec", "ns"),
        _Unit(nanoseconds=1, fraction_scale=None, fraction_in_seconds=False),
    ),
    (
        ("usec", "us", "\u00b5s"),
        _Unit(nanoseconds=1_000, fraction_scale=1_000, fraction_in_seconds=False),
    ),
    (
        ("millis", "msec", "ms"),
        _Unit(
            nanoseconds=1_000_000,
            fraction_scale=1_000_000,
            fraction_in_seconds=False,
        ),
    ),
    (
        ("seconds", "second", "secs", "sec", "s"),
        _Unit(nanoseconds=_SECOND, fraction_scale=_SECOND, fraction_in_seconds=False),
    ),
    (
        ("minutes", "minute", "mins", "min", "m"),
        _Unit(
            nanoseconds=60 * _SECOND,
            fraction_scale=60 * _SECOND,
            fraction_in_seconds=False,
        ),
    ),
    (
        ("hours", "hour", "hrs", "hr", "h"),
        _Unit(
            nanoseconds=3_600 * _SECOND,
            fraction_scale=3_600,
            fraction_in_seconds=True,
        ),
    ),
    (
        ("days", "day", "d"),
        _Unit(
            nanoseconds=86_400 * _SECOND,
            fraction_scale=86_400,
            fraction_in_seconds=True,
        ),
    ),
    (
        ("weeks", "week", "wks", "wk", "w"),
        _Unit(
            nanoseconds=604_800 * _SECOND,
            fraction_scale=604_800,
            fraction_in_seconds=True,
        ),
    ),
    (
        ("months", "month", "M"),
        _Unit(
            nanoseconds=2_630_016 * _SECOND,
            fraction_scale=2_630_016,
            fraction_in_seconds=True,
        ),
    ),
    (
        ("years", "year", "yrs", "yr", "y"),
        _Unit(
            nanoseconds=31_557_600 * _SECOND,
            fraction_scale=31_557_600,
            fraction_in_seconds=True,
        ),
    ),
)

_UNITS: typ.Final[dict[str, _Unit]] = {
    spelling: unit for spellings, unit in _UNIT_SPELLINGS for spelling in spellings
}

#: Each unit's length in seconds, for callers comparing budgets.
_UNIT_SECONDS: typ.Final[dict[str, float]] = {
    spelling: unit.nanoseconds / _SECOND for spelling, unit in _UNITS.items()
}


class TimeoutBudgetError(ValueError):
    """Raised when a configured budget cannot be read as a bound."""


class NextestConfigurationError(TimeoutBudgetError):
    """Raised when the configuration cannot be read at all.

    Separate from a budget that bounds nothing. A file that is not TOML,
    or one declaring no ``slow-timeout`` anywhere, is a configuration
    this contract cannot reason about rather than one whose tiers are in
    the wrong order.
    """


class UnboundedTestError(TimeoutBudgetError):
    """Raised when a ``slow-timeout`` terminates no test.

    ``terminate-after`` is optional, and without it nextest marks a test
    slow and lets it run on, so the configuration parses, reads as
    deliberate, and bounds nothing. Reporting that as a period-long
    budget would put a number on the tier that is missing.
    """


def _read_pair(duration: str, text: str, position: int) -> tuple[int, int]:
    """Return one value-and-unit pair in nanoseconds, and where it ends."""
    match = _DURATION_TOKEN.match(text, position)
    if match is None:
        message = (
            f"unrecognized nextest duration {duration!r}: humantime reads "
            f"a sequence of values, each optionally fractional and each "
            f"followed by a unit, or a bare {_BARE_ZERO!r}"
        )
        raise NextestConfigurationError(message)
    unit = _UNITS.get(match["unit"])
    if unit is None:
        message = (
            f"unrecognized nextest duration {duration!r}: "
            f"{match['unit']!r} is not a unit humantime accepts"
        )
        raise NextestConfigurationError(message)
    # humantime ignores whitespace while it accumulates a number and
    # around the fractional point, so the matched digits can read "1 0"
    # or "1 . 5"; int cannot.
    nanoseconds = int(_digits(match["whole"])) * unit.nanoseconds
    if match["fraction"] is not None:
        nanoseconds += _fraction_nanoseconds(duration, match["fraction"], unit)
    return nanoseconds, match.end()


def _digits(matched: str) -> str:
    """Return a matched digit run with its internal whitespace removed."""
    return "".join(matched.split())


def _fraction_nanoseconds(duration: str, matched: str, unit: _Unit) -> int:
    """Return a fractional part in nanoseconds, as humantime computes it.

    humantime carries the fraction as a numerator over a power of ten
    and divides with a remainder check, so a fraction that is not a
    whole number of the unit's smallest step is an error rather than a
    rounded value.

    Parameters
    ----------
    duration : str
        The whole duration, named in any message raised here rather
        than the fraction, which is not what anybody wrote.
    matched : str
        The digits after the point, whitespace and all.
    unit : _Unit
        The unit the fraction belongs to, which decides both the scale
        and whether the division is over seconds or nanoseconds.

    Returns
    -------
    int
        The fraction's contribution, in nanoseconds.

    Raises
    ------
    NextestConfigurationError
        If humantime would refuse the fraction: on a nanosecond, which
        has no smaller step, or where the division leaves a remainder.

    Examples
    --------
    Half a minute divides exactly, into thirty seconds of nanoseconds:

    >>> _fraction_nanoseconds("1.5m", "5", _UNITS["m"])
    30000000000

    A thousandth of an hour does not, because the division there is
    over whole seconds and 3.6 is not one:

    >>> _fraction_nanoseconds("1.001h", "001", _UNITS["h"])
    Traceback (most recent call last):
    ...
    NextestConfigurationError: unrecognized nextest duration '1.001h': ...
    """
    digits = _digits(matched)
    numerator = int(digits)
    denominator = 10 ** len(digits)
    if unit.fraction_scale is None:
        message = (
            f"unrecognized nextest duration {duration!r}: humantime has no "
            f"step below a nanosecond, so a fractional one is an error"
        )
        raise NextestConfigurationError(message)
    scaled = numerator * unit.fraction_scale
    if scaled % denominator:
        step = "second" if unit.fraction_in_seconds else "nanosecond"
        message = (
            f"unrecognized nextest duration {duration!r}: humantime divides "
            f"exactly, and this fraction is not a whole number of the unit's "
            f"{step}s"
        )
        raise NextestConfigurationError(message)
    quotient = scaled // denominator
    return quotient * _SECOND if unit.fraction_in_seconds else quotient


def seconds(duration: str) -> float:
    """Convert a nextest duration to seconds.

    Parameters
    ----------
    duration : str
        A duration as nextest spells it, such as ``"60s"``.

    Returns
    -------
    float
        The duration in seconds.

    Raises
    ------
    NextestConfigurationError
        If the text is not a duration nextest would accept.
    """
    if duration == _BARE_ZERO:
        return 0.0
    text = duration.strip()
    if not text:
        message = f"unrecognized nextest duration {duration!r}: it is empty"
        raise NextestConfigurationError(message)
    total = 0
    position = 0
    while position < len(text):
        nanoseconds, position = _read_pair(duration, text, position)
        total += nanoseconds
    return total / _SECOND
