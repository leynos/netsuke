"""Reading `.config/nextest.toml` as the runner reads it.

The configuration is parsed with ``tomllib`` rather than matched as
text. A text match finds a key inside a comment, inside a ``filter``
string, or in a table nextest never consults, and reports a budget the
runner does not use.

The reading that matters most is the per-test one. nextest warns once
per ``period`` and terminates after ``terminate-after`` of them, so the
budget is their product. Every period in this repository is 60 s, so a
reader taking the period alone would report a 60 s allowance where the
real figure is 300 s, or 600 s for the two Windows overrides. A
``slow-timeout`` naming no ``terminate-after`` terminates nothing at
all, so that form is refused rather than read as a single period.

See "Test timeouts: the tiers this repository sets" in
``docs/developers-guide.md``.
"""

import tomllib
from itertools import starmap

from nextest_durations import (
    NextestConfigurationError,
    UnboundedTestError,
    seconds,
)
from timeout_budgets import (
    NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS,
    TERMINATION_SAFETY_MARGIN_SECONDS,
)


#: One value-and-unit pair of a humantime duration. nextest parses its
#: durations with `humantime` through `humantime_serde`, which reads a
#: sequence of these and sums them, so `2h 30m` and `1d` are valid and a
#: parser taking one pair would refuse configuration the runner accepts.
def _table(value: object) -> dict[str, object]:
    """Return a parsed value as a table, or an empty one.

    ``tomllib`` returns whatever the document said, so a configuration
    naming a scalar where a table belongs yields nothing here rather
    than raising several frames away, and the assertion that finds no
    budget reports the absence.

    Parameters
    ----------
    value : object
        Any value ``tomllib`` produced.

    Returns
    -------
    dict[str, object]
        The table, or an empty one when the value is not a table.
    """
    return dict(value) if isinstance(value, dict) else {}


def _parsed(config_text: str) -> dict[str, object]:
    """Return the nextest configuration as TOML.

    Parameters
    ----------
    config_text : str
        The nextest configuration file's text.

    Returns
    -------
    dict[str, object]
        The parsed document.

    Raises
    ------
    NextestConfigurationError
        If the text is not valid TOML.
    """
    try:
        return tomllib.loads(config_text)
    except tomllib.TOMLDecodeError as error:
        message = f"the nextest configuration is not valid TOML: {error}"
        raise NextestConfigurationError(message) from error


def _budget_tables(config_text: str) -> list[tuple[str, dict[str, object]]]:
    """Return every table nextest reads a per-test budget from.

    Each profile's own table and each of its ``[[overrides]]`` entries,
    with the dotted path that names it so a failure can say which one is
    at fault.

    Parameters
    ----------
    config_text : str
        The nextest configuration file's text.

    Returns
    -------
    list of tuple
        The dotted path and the table, in file order.
    """
    tables: list[tuple[str, dict[str, object]]] = []
    for name, raw in _table(_parsed(config_text).get("profile")).items():
        profile = _table(raw)
        tables.append((f"profile.{name}", profile))
        overrides = profile.get("overrides")
        entries = overrides if isinstance(overrides, list) else []
        tables.extend(
            (f"profile.{name}.overrides[{index}]", _table(entry))
            for index, entry in enumerate(entries)
        )
    return tables


def _slow_timeouts(config_text: str) -> list[tuple[str, object]]:
    """Return every ``slow-timeout`` the configuration declares.

    Parameters
    ----------
    config_text : str
        The nextest configuration file's text.

    Returns
    -------
    list of tuple
        The dotted path of the declaring table and the value.
    """
    return [
        (path, table["slow-timeout"])
        for path, table in _budget_tables(config_text)
        if "slow-timeout" in table
    ]


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


def _budget_of(path: str, value: object) -> float:
    """Return the per-test budget one ``slow-timeout`` declares.

    Parameters
    ----------
    path : str
        The dotted path of the declaring table, for the message.
    value : object
        The parsed value, a table or a bare duration.

    Returns
    -------
    float
        The budget in seconds.

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


def largest_test_allowance(config_text: str) -> float:
    """Return the longest a single test may run, in seconds.

    nextest warns once per ``period`` and terminates after
    ``terminate-after`` of them, so the budget is their product. This
    repository sets five on Linux and ten on Windows against a 60 s
    period, so reading the period alone would understate the largest
    allowance by a factor of ten.

    Parameters
    ----------
    config_text : str
        The nextest configuration file's text.

    Returns
    -------
    float
        The longest per-test budget.

    A ``slow-timeout`` that names no ``terminate-after`` raises
    :class:`UnboundedTestError` from :func:`_budget_of` rather than
    counting as one period, because such a configuration bounds nothing.

    Raises
    ------
    NextestConfigurationError
        If the configuration declares no ``slow-timeout`` at all.
    """
    budgets = list(starmap(_budget_of, _slow_timeouts(config_text)))
    if not budgets:
        message = (
            "the nextest configuration declares no slow-timeout, so no test "
            "is bounded and there is no per-test tier to compare against"
        )
        raise NextestConfigurationError(message)
    return max(budgets)


def bounds_a_single_test(config_text: str, profile: str = "default") -> bool:
    """Return whether a profile's own table terminates a slow test.

    Only the profile's own ``slow-timeout`` counts. An override bounds
    the tests its filter matches; the profile's own bounds the rest, so
    a profile whose only ``terminate-after`` sits in an override leaves
    every unmatched test running with no bound at all while
    :func:`largest_test_allowance` still reports a comfortable number.

    Parameters
    ----------
    config_text : str
        The nextest configuration file's text.
    profile : str
        The profile to read.

    Returns
    -------
    bool
        True when that profile's own ``slow-timeout`` is a table setting
        ``terminate-after``.
    """
    own = _table(_table(_parsed(config_text).get("profile")).get(profile))
    table = own.get("slow-timeout")
    return isinstance(table, dict) and table.get("terminate-after") is not None


def grace_period(config_text: str) -> float:
    """Return the longest grace period the configuration names, in seconds.

    Read from the configuration rather than fixed, so a profile that
    raised its grace period raises the requirement too. nextest's own
    default applies when none is named, as none is here.

    Parameters
    ----------
    config_text : str
        The nextest configuration file's text.

    Returns
    -------
    float
        The largest configured grace period, or nextest's default.
    """
    periods = [
        seconds(grace)
        for _, value in _slow_timeouts(config_text)
        if isinstance(value, dict)
        and isinstance(grace := value.get("grace-period"), str)
    ]
    return max(periods, default=NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS)


def termination_allowance(config_text: str) -> float:
    """Return the time nextest may take to stop the run, in seconds.

    Two terms, not one. Hitting the whole-run budget starts nextest's
    ordinary termination procedure rather than stopping the run: on Unix
    it signals the process group and waits ``slow-timeout.grace-period``
    before killing it; on Windows termination is immediate and the grace
    period is ignored for timeouts. That grace period is the first term;
    the second is a fixed margin for the teardown and report writing
    that follow it. A single floor over the two would absorb every grace
    period below the margin, so raising one would look free until the
    run it cancelled.

    Parameters
    ----------
    config_text : str
        The nextest configuration file's text.

    Returns
    -------
    float
        The grace period plus the safety margin.
    """
    return grace_period(config_text) + TERMINATION_SAFETY_MARGIN_SECONDS


def global_timeout(config_text: str) -> float | None:
    """Return the whole-run budget, or None when none is set.

    Read from ``[profile.default]`` alone. nextest's other profiles
    inherit that table unless they override it, and an ``[[overrides]]``
    entry cannot carry one, so a value found elsewhere is not the budget
    in force.

    Parameters
    ----------
    config_text : str
        The nextest configuration file's text.

    Returns
    -------
    float or None
        The whole-run budget in seconds, or None when the default
        profile declares none.

    Raises
    ------
    NextestConfigurationError
        If the key is present but is not a duration string. Reading that
        as absent would skip the ordering assertion it exists for.
    """
    profile = _table(_table(_parsed(config_text).get("profile")).get("default"))
    if "global-timeout" not in profile:
        return None
    budget = profile["global-timeout"]
    if not isinstance(budget, str):
        message = (
            f"[profile.default].global-timeout is {budget!r}, which nextest "
            f"refuses: the option is a duration string, so 600 is not a "
            f'shorter way of writing "600s". Reading it as absent would '
            f"skip the whole-run ordering assertion and hide the fault"
        )
        raise NextestConfigurationError(message)
    return seconds(budget)
