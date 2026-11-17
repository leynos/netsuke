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

from actions_expression_evaluator import (
    ExpressionIssue as ExpressionIssue,
)
from actions_expression_evaluator import (
    UnsupportedExpressionError as UnsupportedExpressionError,
)
from actions_expression_evaluator import (
    evaluate_expression as evaluate_expression,
)

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


#: A whole expression holding exactly one single-quoted string literal.
_LITERAL_EXPRESSION = re.compile(r"^\$\{\{\s*'(?P<literal>(?:[^']|'')*)'\s*\}\}$")

#: A whole expression reading one value of the matrix context.
_MATRIX_EXPRESSION = re.compile(r"^\$\{\{\s*matrix\.[A-Za-z_][A-Za-z0-9_-]*\s*\}\}$")


def static_label(value: str) -> str | None:
    """Return the one label a whole literal expression selects, if it does.

    GitHub interpolates an expression anywhere in a string, so
    ``${{ 'private-runner' }}`` selects exactly what ``private-runner``
    selects. An expression that computes its value, a value mixing literals
    and expressions, and a plain literal all return ``None``: only a whole
    expression holding one quoted literal resolves here.

    Parameters
    ----------
    value : str
        A scalar read from a workflow, such as a `runs-on` label.

    Returns
    -------
    str | None
        The quoted literal, with doubled quotes unescaped, or ``None`` when
        the value is not one whole literal expression.

    Examples
    --------
    >>> static_label("${{ 'private-runner' }}")
    'private-runner'
    >>> static_label("${{ 'it''s' }}")
    "it's"
    >>> static_label("${{ matrix.runner }}") is None
    True
    >>> static_label("${{ 'a' }}-${{ 'b' }}") is None
    True
    """
    match = _LITERAL_EXPRESSION.fullmatch(value.strip())
    if match is None:
        return None
    return match["literal"].replace("''", "'") or None


def names_a_matrix_value(value: str) -> bool:
    """Report whether a whole expression reads one matrix value.

    Such an expression names no label of its own: the matrix legs hold the
    labels, and a lane that chooses its runner this way has each leg read
    directly.

    Parameters
    ----------
    value : str
        A scalar read from a workflow, such as a `runs-on` label.

    Returns
    -------
    bool
        ``True`` when the value is exactly one ``matrix.<axis>`` expression.

    Examples
    --------
    >>> names_a_matrix_value("${{ matrix.runner }}")
    True
    >>> names_a_matrix_value("${{ inputs.runner }}")
    False
    >>> names_a_matrix_value("windows-latest")
    False
    """
    return _MATRIX_EXPRESSION.fullmatch(value.strip()) is not None


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


#: One comparison of a context field against a quoted literal.
_COMPARISON = re.compile(
    r"^(?P<context>env|github|steps)\.(?P<field>"
    r"(?:[A-Za-z_][A-Za-z0-9_-]*\.outputs\.)?[A-Za-z_][A-Za-z0-9_-]*"
    r")\s*(?P<operator>==|!=)\s*'(?P<literal>[^']*)'$"
)


def evaluate_conjunction(
    expression: str, contexts: cabc.Mapping[str, cabc.Mapping[str, str]]
) -> bool:
    """Evaluate a top-level conjunction of context comparisons.

    The grammar is exactly what the trunk-only guards use: clauses joined by
    ``&&``, each comparing ``env.NAME``, ``github.FIELD`` or
    ``steps.ID.outputs.NAME`` with ``==`` or ``!=`` against a single-quoted
    literal. A field absent from its context
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
        raise UnsupportedExpressionError(ExpressionIssue.UNPARSABLE_SYNTAX, clause)
    # A step output is named `<id>.outputs.<name>`, and only a step output is;
    # an `env` or `github` field with that shape is not a comparison modelled.
    if (match["context"] == "steps") != (".outputs." in match["field"]):
        raise UnsupportedExpressionError(ExpressionIssue.UNSUPPORTED_COMPARISON, clause)
    value = contexts.get(match["context"], {}).get(match["field"], "")
    equal = value == match["literal"]
    return equal if match["operator"] == "==" else not equal
