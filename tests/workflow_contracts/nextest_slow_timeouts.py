"""Read the per-test budget one ``slow-timeout`` declares.

Split from ``nextest_budgets`` so both stay inside the 400-line limit the
lint gate enforces, following the split that produced
``nextest_durations`` from the same module. The cap is per file, so a
module that reaches it has no room for the next fact anyone needs to
record; the reader below is the half that reads a single declaration.

A ``slow-timeout`` is two numbers, not one. nextest warns once per
``period`` and terminates after ``terminate-after`` of them, so the
budget is their product, and reading the period alone understates it by
its multiplier. Both halves are read from the configuration rather than
assumed: a ``terminate-after`` nextest would refuse is refused here too,
and one that is absent bounds nothing at all, since such a test is
marked slow and left to run on.

See "Test timeouts: the tiers this repository sets" in
``docs/developers-guide.md``.
"""

import typing as typ

from nextest_durations import NextestConfigurationError, UnboundedTestError, seconds

if typ.TYPE_CHECKING:
    import fractions


def _multiplier_of(path: str, multiplier: object) -> int:
    """Return a ``terminate-after`` value, refusing what nextest refuses.

    nextest types it as a `NonZeroUsize`, so a zero, a negative, a
    fraction or a quoted number is a configuration it rejects. Reading
    any of them as a number here would put a budget on a tier the runner
    never applies, and zero in particular would make the per-test
    allowance vanish and every comparison above it pass.

    Booleans are refused before integers because `True` is an `int` in
    Python and would otherwise read as a multiplier of one.

    Parameters
    ----------
    path : str
        The dotted path of the declaring table, for the message.
    multiplier : object
        The value as ``tomllib`` returned it.

    Returns
    -------
    int
        The multiplier.

    Raises
    ------
    NextestConfigurationError
        If the value is not a positive integer.
    """
    match multiplier:
        case bool():
            pass
        case int() as count if count > 0:
            return count
        case _:
            pass
    message = (
        f"{path}.slow-timeout has terminate-after={multiplier!r}, which "
        f"nextest refuses: it is typed as a non-zero positive integer, so a "
        f"zero, a negative, a fraction or a quoted number configures nothing"
    )
    raise NextestConfigurationError(message)


def budget_of(path: str, value: object) -> fractions.Fraction:
    """Return the per-test budget one ``slow-timeout`` declares.

    Parameters
    ----------
    path : str
        The dotted path of the declaring table, for the message.
    value : object
        The parsed value, a table or a bare duration.

    Returns
    -------
    fractions.Fraction
        The budget in seconds, exactly.

    Raises
    ------
    UnboundedTestError
        If the value names no ``terminate-after``, in either spelling.
        nextest then marks the test slow and lets it run on, so there is
        no per-test tier to compare against.
    NextestConfigurationError
        If the value is a table with no ``period``, or is neither a
        table nor a duration.
    """
    match value:
        case str():
            message = (
                f'{path}.slow-timeout = "{value}" sets a warning period with '
                f"no terminate-after, so nextest reports the test as slow and "
                f"never stops it"
            )
            raise UnboundedTestError(message)
        case dict():
            pass
        case _:
            message = f"{path}.slow-timeout is neither a table nor a duration"
            raise NextestConfigurationError(message)
    period = value.get("period")
    if not isinstance(period, str):
        message = f"{path}.slow-timeout names no period: {value!r}"
        raise NextestConfigurationError(message)
    multiplier = value.get("terminate-after")
    if multiplier is None:
        message = (
            f"{path}.slow-timeout sets no terminate-after, so nextest marks "
            f"the test slow and lets it run on; there is no per-test tier to "
            f"compare against"
        )
        raise UnboundedTestError(message)
    return seconds(period) * _multiplier_of(path, multiplier)
