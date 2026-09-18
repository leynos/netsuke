"""Resolving one lane's environment across the scopes GitHub reads.

A workflow may declare a variable on the step, on its job, or on the
document, and the innermost declaration wins. Reading fewer than all
three reports a lane as setting nothing while an outer scope set it,
which for the watchdog means reporting a bounded lane as inheriting the
shared action's undocumented default, and for the nextest profile means
reporting a capped lane as uncapped.

Separated from ``coverage_lanes``, which builds the lanes themselves, so
neither module outgrows the 400-line limit the Python lint gate
enforces.
"""

import fractions
import math
import typing as typ

from timeout_budgets import NEXTEST_PROFILE_VARIABLE, WATCHDOG_VARIABLE


def watchdog_of(
    document: dict[str, typ.Any],
    job: dict[str, typ.Any],
    step: dict[str, typ.Any],
) -> fractions.Fraction | None:
    """Return the watchdog budget in force for one step.

    All three levels are read, innermost first, as GitHub resolves them.
    Both workflows here set the value at job level, so a contract reading
    only the step would find nothing and report every lane as inheriting
    the action's default, which is exactly backwards. A workflow-level
    value would be missed the same way.

    Parameters
    ----------
    document : dict[str, typ.Any]
        The whole workflow document.
    job : dict[str, typ.Any]
        The enclosing job.
    step : dict[str, typ.Any]
        The coverage step.

    Returns
    -------
    float or None
        The budget in seconds, or None when no level sets one.
    """
    return _budget_from(_declared_in_scope(document, job, step, WATCHDOG_VARIABLE))


def _declared_in_scope(
    document: dict[str, typ.Any],
    job: dict[str, typ.Any],
    step: dict[str, typ.Any],
    variable: str,
) -> object:
    """Return the innermost declaration of one variable, or None.

    Step, then job, then workflow, which is the order GitHub resolves
    them in. A reading that stopped at any one of the three would report
    a lane as setting nothing while an outer scope set it.

    A blank or whitespace-only value stops the walk like any other.
    GitHub takes the most specific declaration of a variable, and an
    empty string is a declaration: a step interpolating an expression
    that resolved to nothing hands the process an empty value and the
    job's value never reaches it. Falling through here would report the
    outer budget or profile while the action received neither, which is
    the one reading that turns a lane losing its budget into a pass.
    What a blank means is then the reader's business: each one below
    reports it as nothing set, which is what the consumer sees.

    Parameters
    ----------
    document : dict[str, typ.Any]
        The whole workflow document.
    job : dict[str, typ.Any]
        The enclosing job.
    step : dict[str, typ.Any]
        The step.
    variable : str
        The environment variable's name.

    Returns
    -------
    object
        The value as the YAML parser returned it, or None when no scope
        declares the variable at all. A declared blank is returned as
        the blank it is, not as an absence.
    """
    for owner in (step, job, document):
        environment = owner.get("env")
        if not isinstance(environment, dict):
            continue
        if variable in environment:
            return environment[variable]
    return None


def nextest_profile_of(
    document: dict[str, typ.Any],
    job: dict[str, typ.Any],
    step: dict[str, typ.Any],
) -> str | None:
    """Return the nextest profile one step selects, or None.

    Resolved across all three environment scopes, as the watchdog is.
    Both coverage lanes set the variable at job level, so a reading
    consulting only the step would report every lane as running under
    ``default`` and the whole-run budget as reaching none of them.

    A declared blank reads as the empty string rather than as the
    enclosing scope's value: nextest receives an empty `NEXTEST_PROFILE`
    and selects nothing, so the lane has no whole-run budget however the
    job above it is written.

    Parameters
    ----------
    document : dict[str, typ.Any]
        The whole workflow document.
    job : dict[str, typ.Any]
        The enclosing job.
    step : dict[str, typ.Any]
        The coverage step.

    Returns
    -------
    str or None
        The profile name, or None when no scope selects one.
    """
    raw = _declared_in_scope(document, job, step, NEXTEST_PROFILE_VARIABLE)
    return None if raw is None else str(raw).strip()


class WatchdogValueError(ValueError):
    """Raised when a workflow's watchdog value cannot be read as seconds.

    Distinguished from an unset watchdog rather than folded into it. A
    lane that sets nothing inherits the action's default, which is one
    fault; a lane that sets ``abc`` has an author who meant something
    and got neither, which is another. Reporting the second as the first
    would name the wrong remedy.
    """


def _budget_from(raw: object) -> fractions.Fraction | None:
    """Return the resolved watchdog budget, or None when none is set.

    This reads the one declaration :func:`_declared_in_scope` chose, so
    there is no next source to consider. A blank or whitespace-only
    value, which is what a workflow writes when it interpolates an
    expression that resolved to nothing, reads as no budget: the action
    receives the blank and applies its own default. It does not fall
    through to an outer scope, because that scope's value never reaches
    the action either.

    Anything else that is not a positive number of seconds is refused
    with the value in the message. The shared action reads a
    non-positive value as no timeout at all, so a lane carrying one has
    no third tier while appearing to declare one.

    Parameters
    ----------
    raw : object
        The value the workflow set, as the YAML parser returned it.

    Returns
    -------
    fractions.Fraction or None
        The budget in seconds exactly, or None when the source sets
        none. Exact because it is compared against a sum of budgets
        read from three files, and one float among those terms loses
        the whole comparison silently.

    Raises
    ------
    WatchdogValueError
        If the value is present and non-blank but not a positive number
        of seconds.
    """
    if raw is None:
        return None
    text = str(raw).strip()
    if not text:
        return None
    # Parsed as a float first and converted afterwards. `Fraction` has
    # no notion of `nan` or `inf`: it raises on both, which would make
    # them unreadable text rather than the named refusal below, and a
    # workflow interpolating an expression to `inf` is exactly the case
    # that refusal exists to name. The float here is a parser, not a
    # value: nothing is compared against it before it becomes exact.
    try:
        seconds = float(text)
    except ValueError as error:
        message = (
            f"{WATCHDOG_VARIABLE}={raw!r} is not a number of seconds; the "
            f"lane sets a watchdog its author meant and the action will not "
            f"read"
        )
        raise WatchdogValueError(message) from error
    if not math.isfinite(seconds) or seconds <= 0:
        message = (
            f"{WATCHDOG_VARIABLE}={raw!r} is not a positive, finite number of "
            f"seconds, so the cargo invocation is unbounded while appearing to "
            f"be bounded. `nan` and `inf` parse as floats and pass a `<= 0` "
            f"test, so they are refused by name rather than reaching the "
            f"ceiling arithmetic and failing there"
        )
        raise WatchdogValueError(message)
    return fractions.Fraction(text)
