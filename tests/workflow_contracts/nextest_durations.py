"""Nextest durations, and the faults a configuration can carry.

Split from ``nextest_budgets`` so both stay inside the 400-line limit
the lint gate enforces. nextest parses durations with `humantime`
through `humantime_serde`, which reads a sequence of value-and-unit
pairs and sums them, so `2h 30m` and `1d` are valid and a parser taking
one pair would refuse configuration the runner accepts.
"""

import re
import typing as typ

_DURATION_TOKEN: typ.Final[re.Pattern[str]] = re.compile(
    r"(?P<value>\d+)\s*(?P<unit>[A-Za-z\u00b5]+)\s*"
)

#: Every unit `humantime` accepts, with its length in seconds, using
#: humantime's own definitions of a month and a year. Spelt out in full
#: rather than trimmed to the plausible ones, because refusing a unit
#: nextest accepts would fail a configuration the runner is happy with.
_UNIT_SECONDS: typ.Final[dict[str, float]] = {
    "nsec": 1e-9,
    "ns": 1e-9,
    "usec": 1e-6,
    "us": 1e-6,
    "\u00b5s": 1e-6,
    "msec": 0.001,
    "ms": 0.001,
    "seconds": 1.0,
    "second": 1.0,
    "secs": 1.0,
    "sec": 1.0,
    "s": 1.0,
    "minutes": 60.0,
    "minute": 60.0,
    "mins": 60.0,
    "min": 60.0,
    "m": 60.0,
    "hours": 3600.0,
    "hour": 3600.0,
    "hrs": 3600.0,
    "hr": 3600.0,
    "h": 3600.0,
    "days": 86400.0,
    "day": 86400.0,
    "d": 86400.0,
    "weeks": 604800.0,
    "week": 604800.0,
    "w": 604800.0,
    "months": 2630016.0,
    "month": 2630016.0,
    "M": 2630016.0,
    "years": 31557600.0,
    "year": 31557600.0,
    "y": 31557600.0,
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
    text = duration.strip()
    if not text:
        message = f"unrecognized nextest duration {duration!r}: it is empty"
        raise NextestConfigurationError(message)
    total = 0.0
    position = 0
    while position < len(text):
        match = _DURATION_TOKEN.match(text, position)
        if match is None:
            message = (
                f"unrecognized nextest duration {duration!r}: humantime reads "
                f"a sequence of whole numbers each followed by a unit"
            )
            raise NextestConfigurationError(message)
        unit = match["unit"]
        if unit not in _UNIT_SECONDS:
            message = (
                f"unrecognized nextest duration {duration!r}: {unit!r} is not "
                f"a unit humantime accepts"
            )
            raise NextestConfigurationError(message)
        total += float(match["value"]) * _UNIT_SECONDS[unit]
        position = match.end()
    return total
