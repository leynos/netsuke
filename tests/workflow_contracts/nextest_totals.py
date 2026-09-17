"""humantime's accumulator, and the faults a configuration can carry.

Split from ``nextest_durations`` so both stay inside the 400-line limit
``AGENTS.md`` sets, on the same seam ``nextest_units`` was split on:
this module is humantime's arithmetic and the reader beside it is the
grammar that feeds it. Two of the three families the estate
differential measures live here, the 64-bit range in :func:`u64` and
the carry in :class:`Total`; the character classes stay with the
grammar, which is where they are read.

The error classes are here because every check in this module raises
one, and the arithmetic must not import the reader to do it.
``nextest_durations`` re-exports all three, so a reader of a budget
still imports them from there.
"""

import fractions
import typing as typ

from nextest_units import SECOND as _SECOND

#: The largest value humantime's parser can hold. Its accumulators and
#: its intermediate products are all ``u64``, checked at every step,
#: and Python's integers are not, so the checks are made explicitly
#: here. Without them the reader accepts fourteen of the differential's
#: range inputs that nextest refuses at startup.
U64_MAX: typ.Final[int] = 2**64 - 1


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


def u64(duration: str, value: int) -> int:
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
    if value > U64_MAX:
        message = (
            f"unrecognized nextest duration {duration!r}: humantime "
            f"accumulates in 64-bit integers and this exceeds their range"
        )
        raise NextestConfigurationError(message)
    return value


class Total:
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
    >>> total = Total()
    >>> total.add("1s 1s", 1, 0)
    >>> total.add("1s 1s", 1, 0)
    >>> total.as_seconds()
    Fraction(2, 1)
    >>> float(total.as_seconds())
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
        nanos = u64(duration, self.nanoseconds + nanoseconds)
        running = u64(duration, self.seconds + seconds)
        # humantime carries in two places, not one. Its `add_current`
        # normalizes on a strict `>`, so a nanosecond part of exactly
        # one second survives that step untouched, and the same
        # function then ends with `Duration::new`, which carries it on
        # `>=` and aborts the process rather than erroring when that
        # carry overflows. nextest cannot run either way, so a refusal
        # here answers both.
        #
        # The strict one is therefore unreachable from outside: every
        # part goes through `add_current`, and each call ends by
        # carrying an exact second, so the running total humantime
        # offers the next part never holds one. The two were written
        # out separately here at first and collapsing them changed no
        # answer over any of the differential's inputs; an
        # unfalsifiable guard is worse than none. `1000000000ns
        # 18446744073709551615ns` is the input that would tell them
        # apart if the carry were deferred to the end of the parse:
        # humantime reads it as 18446744074.709551615 s, because the
        # first part is already a whole second by the time the second
        # arrives. It is in the contract for that reason. This single
        # `>=` is what the evidence supports, and it is what makes
        # `0.5s 0.5s` one second while
        # `18446744073709551615s 500ms 500ms` is refused.
        if nanos >= _SECOND:
            running = u64(duration, running + nanos // _SECOND)
            nanos %= _SECOND
        self.seconds = running
        self.nanoseconds = nanos

    def as_seconds(self) -> fractions.Fraction:
        """Return the total in seconds, exactly.

        A ``Fraction`` rather than a ``float`` because these values are
        compared with each other rather than merely printed. humantime's
        range reaches 2**64 seconds and a float carries 53 bits of
        significand, so above 2**53 it cannot hold two budgets that
        differ by a second: ``18446744073709551614s`` and
        ``18446744073709551615s`` are both inputs in the differential
        this reader is measured against, and both convert to the same
        float. An ordering assertion between them would compare equal
        and pass whichever way round it was written.

        The nanosecond part makes the same point at the other end.
        ``0.1s`` has no exact float, so a budget assembled from tenths
        and one written as a decimal would differ by a rounding error
        rather than by anything anyone configured.

        Returns
        -------
        fractions.Fraction
            Whole seconds and the nanosecond part together, exactly.
        """
        return fractions.Fraction(self.seconds) + fractions.Fraction(
            self.nanoseconds, _SECOND
        )
