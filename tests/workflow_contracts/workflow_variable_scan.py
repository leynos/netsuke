"""Find ``vars.`` references a workflow cannot actually resolve.

A GitHub Actions expression naming a repository variable that the repository
has not declared does not fail. It interpolates to the empty string and the
step runs with a value the author did not intend, which is worse than a
missing feature: the workflow reads as though it configured something. The
observed instance was an upload step handed
``${{ vars.CODESCENE_CLI_SHA256 }}`` as an installer checksum, where the empty
value made the check a no-op that looked like verification.

These predicates answer the question for one parsed step, over every string it
holds. The scan is deliberately not restricted to the ``with`` block: an ``if``
condition decides whether a step runs at all, so a ``vars.`` reference there is
the most consequential place one can appear, and an ``env`` entry is where a
credential would be bound.

They read parsed values rather than files, so the callers can hold the
repository's own workflows to the contract and drive shapes the repository does
not have.

Run via ``make test-workflow-contracts``.
"""

import re
import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: Matches any `vars.<NAME>` expression, wherever it appears in a value. The
#: repository declares no variables, so every one of them resolves to the empty
#: string.
REPOSITORY_VARIABLE_EXPRESSION: typ.Final[re.Pattern[str]] = re.compile(
    r"\$\{\{\s*vars\."
)

#: The one repository variable the workflows legitimately read, and the one
#: place a `vars.` expression is allowed to appear in the coverage lane: it
#: selects between the two sccache backends. Every other `vars.` reference is a
#: value that silently resolves to the empty string.
PERMITTED_VARIABLE_NAME: typ.Final[str] = "NETSUKE_SCCACHE_LOCAL_DIR"


def unbound_variable_references(step: dict[str, object]) -> list[str]:
    """Return every ``vars.`` reference in a step but the permitted one.

    Parameters
    ----------
    step
        One parsed workflow step. Every string it holds is scanned, so a
        reference in ``if``, ``env``, or ``with`` is found alike.

    Returns
    -------
    list[str]
        One entry per offending value, empty when the step reads no repository
        variable or reads only the one this repository defines.
    """
    offending: list[str] = []
    for value in _iter_strings(step):
        for match in REPOSITORY_VARIABLE_EXPRESSION.finditer(value):
            name = _variable_name(value, match.end())
            if name != PERMITTED_VARIABLE_NAME:
                offending.append(value)
    return offending


def _variable_name(value: str, start: int) -> str:
    """Return the variable name beginning at ``start`` in an expression."""
    match = re.match(r"[A-Za-z_][A-Za-z0-9_]*", value[start:])
    return match.group(0) if match else ""


def _iter_strings(value: object) -> cabc.Iterator[str]:
    """Yield every string nested anywhere in a parsed YAML value."""
    match value:
        case str() as text:
            yield text
        case dict() as mapping:
            for key, item in mapping.items():
                yield from _iter_strings(key)
                yield from _iter_strings(item)
        case list() as sequence:
            for item in sequence:
                yield from _iter_strings(item)
        case _:
            return
