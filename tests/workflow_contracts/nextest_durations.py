r"""Nextest durations, and the faults a configuration can carry.

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
the arithmetic differs by unit: see ``nextest_units``. Reading them as
floats accepted four shapes humantime refuses: `0.5ns`, `1.5ns`,
`0.123h` and `0.0000000001s`. Accepting what nextest refuses is the
same fault as refusing what it accepts: the contract would assert an
ordering over a budget the runner never started with.

Every disagreement this reader has had with humantime has been in that
direction. The estate differential measures it over seventy-one
inputs; this reader read twenty-two against that set before the work
below and reads none after. The three families, each answered in a
different place: the character classes, because Python's
``\s`` and ``\d`` are both wider than humantime's, in ``_SPACE_CHARS``
and ``_DIGIT_CHARS``; the 64-bit range, in ``_u64``; and the carry, in
``_Total``. "Test timeouts: the tiers this repository sets" in
``docs/developers-guide.md`` sets out all three with their inputs.
"""

import re
import string
import typing as typ

from nextest_units import SECOND as _SECOND
from nextest_units import UNITS as _UNITS
from nextest_units import Scaling as _Scaling
from nextest_units import Unit as _Unit

#: The characters humantime treats as whitespace, enumerated.
#:
#: humantime skips on Rust's ``char::is_whitespace``, which is the
#: Unicode White_Space property. Python's ``\s`` is that property plus
#: U+001C to U+001F, the file, group, record and unit separators, and
#: ``str.strip`` and ``str.split`` carry the same four-character
#: excess. A reader spelling the class ``\s`` therefore reads
#: ``1\x1cs`` as one second and ``\x1c45m`` as forty-five minutes,
#: both of which nextest refuses at startup. That is the
#: accept-what-the-runner-refuses direction this module exists to
#: avoid, which is why the class is written out rather than
#: abbreviated. ``whitespace_class_is_rusts_own_test`` pins the
#: difference in both directions, so a change in either language's
#: notion of whitespace fails there rather than in a runner.
_SPACE_CHARS: typ.Final[str] = (
    "\t\n\v\f\r \x85\xa0\u1680"
    "\u2000\u2001\u2002\u2003\u2004\u2005"
    "\u2006\u2007\u2008\u2009\u200a"
    "\u2028\u2029\u202f\u205f\u3000"
)

#: The same set as a regular-expression character class.
_SPACE: typ.Final[str] = f"[{re.escape(_SPACE_CHARS)}]"

#: humantime's parser ignores whitespace while it accumulates a
#: number, so `1 0s` is ten seconds rather than a malformed duration.
#:
#: The digits humantime reads, enumerated for the same reason the
#: whitespace above is.
#:
#: Python's ``\d`` matches every Unicode decimal digit. humantime
#: matches ``'0'..='9'`` and nothing else, so an Arabic-Indic or
#: Devanagari numeral is a duration a ``\d`` reader converts happily
#: and nextest refuses at startup: the same direction again, and
#: ``1\u0660s`` is the shape nobody would notice in a file.
#: ``digit_class_is_humantimes_own_test`` pins this set in both
#: directions against Python's.
_DIGIT_CHARS: typ.Final[str] = string.digits

#: The same set as a regular-expression character class.
_DIGIT: typ.Final[str] = f"[{re.escape(_DIGIT_CHARS)}]"

#: Digits with whitespace tolerated between them.
_SPACED_DIGITS: typ.Final[str] = rf"{_DIGIT}(?:{_SPACE}*{_DIGIT})*"

#: One value-and-unit pair. The fractional part is optional and
#: humantime tolerates whitespace around the point; a leading point, a
#: trailing point, a second point, a sign and a digit separator are all
#: refused there and so are refused here. The whole part and the
#: fraction are captured apart because humantime scales them
#: differently.
_DURATION_TOKEN: typ.Final[re.Pattern[str]] = re.compile(
    rf"(?P<whole>{_SPACED_DIGITS})"
    rf"(?:{_SPACE}*\.{_SPACE}*(?P<fraction>{_SPACED_DIGITS}))?"
    rf"{_SPACE}*(?P<unit>[A-Za-z\u00b5]+){_SPACE}*"
)

#: The one duration humantime accepts with no unit. Its parser
#: special-cases the exact text before reading a single character, so
#: the comparison here is against the raw value rather than a stripped
#: one: ``" 0 "`` is not this case and nextest refuses it.
_BARE_ZERO: typ.Final[str] = "0"

#: The largest value humantime's parser can hold. Its accumulators and
#: its intermediate products are all ``u64``, checked at every step,
#: and Python's integers are not, so the checks are made explicitly
#: here. Without them the reader accepts fourteen of the differential's
#: range inputs that nextest refuses at startup.
_U64_MAX: typ.Final[int] = 2**64 - 1


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


def _read_pair(duration: str, text: str, position: int, total: _Total) -> int:
    """Add one value-and-unit pair to the total, and return where it ends."""
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
    whole = _u64(duration, int(_digits(match["whole"])))
    _add_scaled(duration, total, whole, unit.whole)
    if match["fraction"] is not None:
        _add_fraction(duration, total, match["fraction"], unit)
    return match.end()


def _add_scaled(duration: str, total: _Total, value: int, scaling: _Scaling) -> None:
    """Scale one part by its unit and add it, in the unit it lands in."""
    _add_landed(duration, total, _u64(duration, value * scaling.scale), scaling)


def _add_landed(duration: str, total: _Total, amount: int, scaling: _Scaling) -> None:
    """Add an already-scaled amount to whichever part it belongs in."""
    if scaling.in_seconds:
        total.add(duration, amount, 0)
    else:
        total.add(duration, 0, amount)


def _u64(duration: str, value: int) -> int:
    """Return a value humantime could hold, or refuse it as it does.

    Every multiplication and addition in its parser is checked against
    this bound, and Python's integers are not, so each of those steps
    passes through here.

    Returns
    -------
    int
        The value, unchanged, when humantime could hold it.

    Raises
    ------
    NextestConfigurationError
        If the value exceeds what a 64-bit unsigned integer holds, as
        every one of humantime's own checked steps would.
    """
    if value > _U64_MAX:
        message = (
            f"unrecognized nextest duration {duration!r}: humantime "
            f"accumulates in 64-bit integers and this exceeds their range"
        )
        raise NextestConfigurationError(message)
    return value


def _digits(matched: str) -> str:
    """Return a matched digit run with its internal whitespace removed.

    Removes exactly the characters the pattern tolerated. ``str.split``
    would additionally drop U+001C to U+001F, which the pattern never
    let through, so a separator inside a number would vanish here
    rather than be refused. That site is unreachable while the pattern
    refuses those four, and is written this way so a later widening of
    the pattern cannot turn a refusal into a silently different number.

    Returns
    -------
    str
        The digits, with the tolerated whitespace gone and everything
        else left where it was.
    """
    return re.sub(_SPACE, "", matched)


def _add_fraction(duration: str, total: _Total, matched: str, unit: _Unit) -> None:
    """Add a fractional part, or refuse it as humantime does.

    humantime carries the fraction as a numerator over a power of ten
    and divides with a remainder check, so a fraction that is not a
    whole number of the unit's smallest step is an error rather than a
    rounded value.

    The denominator is a power of ten built one digit at a time, and
    that too is checked: a fraction of twenty digits overflows it where
    one of nineteen does not, whatever the digits are, which is why
    ``1.0000000000000000000s`` is one second and
    ``1.00000000000000000000s`` is refused.

    Parameters
    ----------
    duration : str
        The whole duration, named in any message raised here rather
        than the fraction, which is not what anybody wrote.
    total : _Total
        The running total the fraction is added to.
    matched : str
        The digits after the point, whitespace and all.
    unit : _Unit
        The unit the fraction belongs to, which decides both the scale
        and whether the division is over seconds or nanoseconds.

    Raises
    ------
    NextestConfigurationError
        If humantime would refuse the fraction: on a nanosecond, which
        has no smaller step, where the division leaves a remainder, or
        where any step leaves 64 bits.

    Examples
    --------
    Half a minute divides exactly, into thirty seconds of nanoseconds:

    >>> total = _Total()
    >>> _add_fraction("1.5m", total, "5", _UNITS["m"])
    >>> total.as_seconds()
    30.0

    A thousandth of an hour does not, because the division there is
    over whole seconds and 3.6 is not one:

    >>> _add_fraction("1.001h", _Total(), "001", _UNITS["h"])
    ... # doctest: +ELLIPSIS
    Traceback (most recent call last):
    ...
    nextest_durations.NextestConfigurationError: ...'1.001h'...
    """
    scaling = unit.fraction
    if scaling is None:
        message = (
            f"unrecognized nextest duration {duration!r}: humantime has no "
            f"step below a nanosecond, so a fractional one is an error"
        )
        raise NextestConfigurationError(message)
    digits = _digits(matched)
    numerator = _u64(duration, int(digits))
    denominator = _u64(duration, 10 ** len(digits))
    scaled = _u64(duration, numerator * scaling.scale)
    if scaled % denominator:
        step = "second" if scaling.in_seconds else "nanosecond"
        message = (
            f"unrecognized nextest duration {duration!r}: humantime divides "
            f"exactly, and this fraction is not a whole number of the unit's "
            f"{step}s"
        )
        raise NextestConfigurationError(message)
    _add_landed(duration, total, scaled // denominator, scaling)


class _Total:
    """The running total humantime keeps: whole seconds and nanoseconds.

    Both are 64-bit unsigned there and every step is checked, so this
    carries the pair rather than a single count of nanoseconds. The two
    are not the same claim. ``18446744073709551615ns`` twice over names
    about 36.9 billion seconds, some 1,169 years, which is nowhere near
    the seconds ceiling, and humantime refuses it all the same because
    the second value overflows the nanosecond accumulator before it is
    carried. A reader checking only an accumulated total finds that
    comfortably in range and accepts it.

    Examples
    --------
    >>> total = _Total()
    >>> total.add("1s 1s", 1, 0)
    >>> total.add("1s 1s", 1, 0)
    >>> total.as_seconds()
    2.0
    """

    def __init__(self) -> None:
        self.seconds = 0
        self.nanoseconds = 0

    def add(self, duration: str, seconds: int, nanoseconds: int) -> None:
        """Add one part, refusing what humantime's checks would refuse.

        Every sum and every carry goes through ``_u64``, which raises
        where humantime's own checked step would, so a part that does
        not fit is refused here rather than accumulated.
        """
        nanos = _u64(duration, self.nanoseconds + nanoseconds)
        carried = seconds
        if nanos > _SECOND:
            carried = _u64(duration, carried + nanos // _SECOND)
            nanos %= _SECOND
        running = _u64(duration, self.seconds + carried)
        # humantime normalizes on a strict `>`, so a nanosecond part of
        # exactly one second reaches `Duration::new`, which carries it
        # and aborts the process rather than returning an error when
        # that carry overflows. nextest cannot run either way, so the
        # refusal here is the same. This is why `0.5s 0.5s` is one
        # second while `18446744073709551615s 500ms 500ms` is refused:
        # one mechanism, two answers.
        if nanos >= _SECOND:
            running = _u64(duration, running + nanos // _SECOND)
            nanos %= _SECOND
        self.seconds = running
        self.nanoseconds = nanos

    def as_seconds(self) -> float:
        """Return the total in seconds.

        Returns
        -------
        float
            Whole seconds and the nanosecond part together.
        """
        return self.seconds + self.nanoseconds / _SECOND


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

    Examples
    --------
    >>> seconds("45m")
    2700.0
    >>> seconds("2h 30m")
    9000.0
    >>> seconds("0.5s 0.5s")
    1.0
    """
    if duration == _BARE_ZERO:
        return 0.0
    text = duration.strip(_SPACE_CHARS)
    if not text:
        message = f"unrecognized nextest duration {duration!r}: it is empty"
        raise NextestConfigurationError(message)
    total = _Total()
    position = 0
    while position < len(text):
        position = _read_pair(duration, text, position, total)
    return total.as_seconds()
