"""Read the gate's environment variables in Bash's own two default forms.

Both forms appear in the shell this gate replaces and neither is interchangeable
with the other, so both are defined once, here, and every module that reads an
optional variable reads it through the form that variable was read with.

``optional`` is Bash's ``${VAR:-default}``, the colon form, which substitutes
when the variable is unset *or empty*. The shell used it for every adapter,
path, sink, and mode value.

``defaulted_only_when_unset`` is ``${VAR-default}``, with no colon, which the
shell wrote for ``NETSUKE_RELEASE_ADMISSION_ENFORCE`` alone. An exported empty
value was therefore kept and refused by the gate's own mode check rather than
replaced by the default, and that refusal is load-bearing: substituting
observation mode for a refused enforcement mode would let a configuration the
gate rejects run as though it had been accepted.

Nothing here reads the process environment itself. Each reader takes the mapping
it resolves, which is what lets the resolution be exercised without a child
process and what keeps the choice of "which environment" at the call site that
owns it -- ``os.environ`` in the entry point, a constructed mapping in a test.

This module exists separately from the rest of ``configuration`` so that the
whole-gate configuration module stays inside the repository's 400-line ceiling
while both default forms keep one home. Callers import it as a module, never as
a name: ``environment.optional`` says which form it is, which
``from .environment import optional`` would not.
"""

import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc


def optional(environment: cabc.Mapping[str, str], variable: str, default: str) -> str:
    """Return a variable's value, or *default* when it is unset or empty.

    Returns
    -------
    str
        The configured value, or the default when it is unset or empty.

    Examples
    --------
    >>> optional({"A": ""}, "A", "fallback")
    'fallback'
    >>> optional({"A": "set"}, "A", "fallback")
    'set'
    """
    return environment.get(variable) or default


def defaulted_only_when_unset(
    environment: cabc.Mapping[str, str], variable: str, default: str
) -> str:
    """Return a variable's value, or *default* only when it is unset.

    :meth:`~collections.abc.Mapping.get` happens to agree with this form,
    because it substitutes only for an *unset* variable too; the membership
    test is what states that, so a reader cannot mistake this helper for the
    colon form above it.

    Returns
    -------
    str
        The configured value, empty when it was exported empty, or the default
        when the variable is unset.

    Examples
    --------
    >>> defaulted_only_when_unset({"A": ""}, "A", "fallback")
    ''
    >>> defaulted_only_when_unset({}, "A", "fallback")
    'fallback'
    """
    if variable not in environment:
        return default
    return environment[variable]
