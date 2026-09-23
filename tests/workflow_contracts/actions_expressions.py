"""Reads the structure of a GitHub Actions ``if`` expression, not its text.

Two trunk-only guards depend on it: the cache save in
``runner_placement_invariants`` and the CodeScene upload in
``ci_coverage_wiring_invariants``. Both ask structural questions of an
expression. Does it contain a disjunction? Which clauses does it conjoin at
the top level? A text search answers the wrong question. A ``&&`` or ``||``
inside a string literal is not an operator, and a clause inside a
parenthesized group, possibly negated, is not a top-level conjunct.

An Actions string literal is single-quoted, and a quote inside it is written
twice (``'it''s'``). A double quote is not a string delimiter in the
expression language. It is treated as one anyway, so that an operator
inside it is hidden from neither check. Such an expression fails to parse on
GitHub and never runs, so reading it either way cannot pass a guard GitHub
would honour.

Run via ``make test-workflow-contracts``.
"""

import re
import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: The characters that open a string literal, matched by the same character.
_QUOTES: typ.Final[str] = "'\""


def _unquoted_positions(expression: str) -> cabc.Iterator[tuple[int, int]]:
    """Yield each index outside a string literal, with its bracket depth.

    A doubled quote inside a literal, the escaped quote, needs no case of its
    own. It closes the literal and reopens it at once, so no character between
    the two quotes is ever yielded.

    Yields
    ------
    tuple[int, int]
        The index of each character outside every literal, and the bracket
        depth after reading it.
    """
    quote: str | None = None
    depth = 0
    index = 0
    while index < len(expression):
        char = expression[index]
        if quote is not None:
            if char == quote:
                quote = None
        elif char in _QUOTES:
            quote = char
        else:
            depth += {"(": 1, ")": -1}.get(char, 0)
            yield index, depth
        index += 1


def contains_unquoted_or(expression: str) -> bool:
    """Report whether `||` appears anywhere outside a string literal.

    `&&` binds tighter than `||` in an Actions expression, so a single
    disjunct anywhere, at any depth, can authorize the step alone.

    Parameters
    ----------
    expression : str
        An Actions ``if`` expression, without the ``${{ }}`` wrapper.

    Returns
    -------
    bool
        ``True`` when a ``||`` occurs outside every string literal.

    Examples
    --------
    >>> contains_unquoted_or("github.ref == 'a' || true")
    True
    >>> contains_unquoted_or("github.ref == 'a||b'")
    False
    """
    return any(
        expression.startswith("||", index)
        for index, _ in _unquoted_positions(expression)
    )


def top_level_conjuncts(expression: str) -> list[str]:
    """Return the clauses an expression conjoins at the top level, stripped.

    A ``&&`` inside a string literal or inside parentheses is not split on. So
    ``true && !(a && b)`` yields two clauses, ``true`` and ``!(a && b)``, and
    neither ``a`` nor ``b`` counts as a clause of the whole.

    Parameters
    ----------
    expression : str
        An Actions ``if`` expression, without the ``${{ }}`` wrapper.

    Returns
    -------
    list[str]
        The top-level conjuncts in order, each stripped of surrounding
        whitespace.

    Examples
    --------
    >>> top_level_conjuncts("a == 'x && y' && (b && c) && d")
    ["a == 'x && y'", '(b && c)', 'd']
    """
    cuts = [
        index
        for index, depth in _unquoted_positions(expression)
        if depth == 0 and expression.startswith("&&", index)
    ]
    bounds = zip([0, *(cut + 2 for cut in cuts)], [*cuts, len(expression)], strict=True)
    return [expression[start:end].strip() for start, end in bounds]


class UnsupportedExpressionError(ValueError):
    """Raised when an expression leaves the grammar the evaluator models.

    An evaluator that answered False outside its grammar would let a guard it
    cannot read pass as one that never runs, so it refuses instead.
    """


#: One comparison of a context field against a quoted literal.
_COMPARISON = re.compile(
    r"^(?P<context>env|github)\.(?P<field>[A-Za-z_][A-Za-z0-9_]*)\s*"
    r"(?P<operator>==|!=)\s*'(?P<literal>[^']*)'$"
)


def evaluate_conjunction(
    expression: str, contexts: cabc.Mapping[str, cabc.Mapping[str, str]]
) -> bool:
    """Evaluate a top-level conjunction of context comparisons.

    The grammar is exactly what the trunk-only guards use: clauses joined by
    ``&&``, each comparing ``env.NAME`` or ``github.FIELD`` with ``==`` or
    ``!=`` against a single-quoted literal. A field absent from its context
    reads as the empty string, as GitHub reads an unset value. Every clause is
    evaluated, so an unsupported one after a false one still refuses, and a
    clause holding a `||` is not a comparison, so a disjunction is refused
    too.

    Parameters
    ----------
    expression : str
        An Actions ``if`` expression, without the ``${{ }}`` wrapper.
    contexts : Mapping[str, Mapping[str, str]]
        The ``env`` and ``github`` values to evaluate against.

    Returns
    -------
    bool
        Whether the step would run. A disjunction or any clause outside the
        grammar propagates ``UnsupportedExpressionError`` from the clause
        reader.

    Examples
    --------
    >>> evaluate_conjunction(
    ...     "env.T != '' && github.ref == 'refs/heads/main'",
    ...     {"env": {"T": "x"}, "github": {"ref": "refs/heads/main"}},
    ... )
    True
    """
    # No separate `||` check: a disjunction left in a conjunct is not a
    # comparison, so the clause grammar below refuses it.
    results = [_compare(clause, contexts) for clause in top_level_conjuncts(expression)]
    return all(results)


def _compare(clause: str, contexts: cabc.Mapping[str, cabc.Mapping[str, str]]) -> bool:
    """Evaluate one comparison clause, refusing anything else."""
    match = _COMPARISON.fullmatch(clause)
    if match is None:
        raise UnsupportedExpressionError(clause)
    value = contexts.get(match["context"], {}).get(match["field"], "")
    equal = value == match["literal"]
    return equal if match["operator"] == "==" else not equal
