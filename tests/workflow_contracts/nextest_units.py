"""humantime's unit table, and how each unit's arithmetic lands.

Split from ``nextest_durations`` so both stay inside the 400-line limit
``AGENTS.md`` sets. The seam is data against behaviour: this module is
what humantime calls each unit and what each one is worth, and the
reader beside it is the grammar that consumes them.

Spelt out in full rather than trimmed to the plausible spellings,
because refusing a unit nextest accepts fails a configuration the
runner is happy with, which is a worse failure than the one the
contract exists to catch.
"""

import typing as typ

#: Nanoseconds in a second, which is the smaller of the two scales
#: humantime's accumulator carries.
SECOND: typ.Final[int] = 1_000_000_000


class Scaling(typ.NamedTuple):
    """One of humantime's products: a multiplier and where it lands.

    humantime multiplies in the unit the product lands in rather than
    in nanoseconds throughout, so a year's whole part scales by
    31,557,600 and the 64-bit check that follows is a check on seconds.
    Scaling in nanoseconds instead would refuse ``584542046090y``,
    which humantime accepts, because that duration is about 1.8e28
    nanoseconds and nothing like that fits in 64 bits. Carrying the
    multiplier and its landing place together is what keeps that
    pairing from coming apart.

    Attributes
    ----------
    scale : int
        What the value is multiplied by.
    in_seconds : bool
        Whether the product is seconds rather than nanoseconds.
    """

    scale: int
    in_seconds: bool


class Unit(typ.NamedTuple):
    """One humantime unit, as its parser treats it.

    The two scalings differ, and not only in magnitude: a second's
    whole part scales by one into seconds while its fraction scales by
    a thousand million into nanoseconds, and the landing place moves at
    a different unit for each. Whole parts land in seconds from a
    second upwards; fractions land in seconds only from an hour
    upwards, which is why ``0.123h`` is refused where ``0.123s`` is
    exact.

    Attributes
    ----------
    whole : Scaling
        How the value's whole part is scaled.
    fraction : Scaling or None
        How a fraction's numerator is scaled before the exact division
        humantime requires, or None when the unit admits no fraction at
        all. ``ns`` is that case: humantime refuses a fractional
        nanosecond outright rather than rounding it.
    """

    whole: Scaling
    fraction: Scaling | None


#: Every spelling humantime accepts, grouped by the unit it names, with
#: humantime's own definitions of a month and a year.
_UNIT_SPELLINGS: typ.Final[tuple[tuple[tuple[str, ...], Unit], ...]] = (
    (("nanos", "nsec", "ns"), Unit(Scaling(1, in_seconds=False), None)),
    (
        ("usec", "us", "\u00b5s"),
        Unit(Scaling(1_000, in_seconds=False), Scaling(1_000, in_seconds=False)),
    ),
    (
        ("millis", "msec", "ms"),
        Unit(
            Scaling(1_000_000, in_seconds=False), Scaling(1_000_000, in_seconds=False)
        ),
    ),
    (
        ("seconds", "second", "secs", "sec", "s"),
        Unit(Scaling(1, in_seconds=True), Scaling(SECOND, in_seconds=False)),
    ),
    (
        ("minutes", "minute", "mins", "min", "m"),
        Unit(Scaling(60, in_seconds=True), Scaling(60 * SECOND, in_seconds=False)),
    ),
    (
        ("hours", "hour", "hrs", "hr", "h"),
        Unit(Scaling(3_600, in_seconds=True), Scaling(3_600, in_seconds=True)),
    ),
    (
        ("days", "day", "d"),
        Unit(Scaling(86_400, in_seconds=True), Scaling(86_400, in_seconds=True)),
    ),
    (
        ("weeks", "week", "wks", "wk", "w"),
        Unit(Scaling(604_800, in_seconds=True), Scaling(604_800, in_seconds=True)),
    ),
    (
        ("months", "month", "M"),
        Unit(Scaling(2_630_016, in_seconds=True), Scaling(2_630_016, in_seconds=True)),
    ),
    (
        ("years", "year", "yrs", "yr", "y"),
        Unit(
            Scaling(31_557_600, in_seconds=True), Scaling(31_557_600, in_seconds=True)
        ),
    ),
)

#: Each spelling humantime accepts, against the unit it names.
UNITS: typ.Final[dict[str, Unit]] = {
    spelling: unit for spellings, unit in _UNIT_SPELLINGS for spelling in spellings
}
