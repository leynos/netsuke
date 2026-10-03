"""Property-test composed Actions expressions against a direct model."""

import collections.abc as cabc

import pytest
from actions_expressions import (
    ExpressionIssue,
    UnsupportedExpressionError,
    evaluate_expression,
)
from hypothesis import given
from hypothesis import strategies as st

type _ExpressionNode = tuple[object, ...]
type _ExpressionContexts = dict[str, object]


_PATH_KEY = st.text(alphabet="abc123-", min_size=1, max_size=6)
_EXPRESSION_STRING = st.text(alphabet="abC -'", max_size=5)
_EXPRESSION_SCALAR = st.one_of(st.booleans(), _EXPRESSION_STRING)


@st.composite
def _expression_cases(
    draw: st.DrawFn,
) -> tuple[_ExpressionNode, _ExpressionContexts, bool, bool]:
    """Generate a bounded expression tree and all paths in its context."""
    input_key = draw(_PATH_KEY)
    dependency_keys = draw(st.lists(_PATH_KEY, min_size=1, max_size=3, unique=True))
    contexts: _ExpressionContexts = {
        "inputs": {input_key: draw(_EXPRESSION_SCALAR)},
        "github": {"event_name": draw(_EXPRESSION_STRING)},
        "steps": {
            "release_modes": {
                "outputs": {"dry-run": draw(st.sampled_from(("true", "false")))}
            }
        },
        "status": {
            "cancelled": draw(st.booleans()),
            "failure": draw(st.booleans()),
            "success": draw(st.booleans()),
        },
        "needs": {
            key: {"result": draw(st.sampled_from(("success", "failure", "skipped")))}
            for key in dependency_keys
        },
    }
    leaves = st.one_of(
        st.booleans().map(lambda value: ("literal", value)),
        _EXPRESSION_STRING.map(lambda value: ("literal", value)),
        st.just(("input", input_key)),
        st.sampled_from([("need", key) for key in dependency_keys]),
        st.just(("github-event",)),
        st.just(("dry-run",)),
        st.sampled_from([
            ("status", name) for name in ("always", "cancelled", "success", "failure")
        ]),
        st.booleans().map(lambda value: ("from-json", value)),
    )
    operators = st.sampled_from(("and", "or", "equal", "not-equal"))
    nodes = st.recursive(
        leaves,
        lambda children: st.one_of(
            children.map(lambda child: ("not", child)),
            children.map(lambda child: ("group", child)),
            st.tuples(st.sampled_from(("and", "or")), children, children),
            st.tuples(operators, leaves, leaves),
        ),
        max_leaves=9,
    )
    return draw(nodes), contexts, draw(st.booleans()), draw(st.booleans())


def _expression_source(node: _ExpressionNode) -> str:
    """Render a generated expression tree without changing its grouping."""
    kind = _node_text(node, 0)
    renderers = {
        "literal": lambda: _render_literal(node[1]),
        "input": lambda: f"inputs['{_node_text(node, 1)}']",
        "need": lambda: f"needs['{_node_text(node, 1)}'].result",
        "github-event": lambda: "github.event_name",
        "dry-run": lambda: "steps.release_modes.outputs['dry-run']",
        "status": lambda: f"{_node_text(node, 1)}()",
        "from-json": lambda: f"fromJSON('{str(node[1]).lower()}')",
    }
    renderer = renderers.get(kind)
    return renderer() if renderer is not None else _render_composite_source(node)


def _render_literal(value: object) -> str:
    """Render a generated boolean or string literal."""
    if isinstance(value, bool):
        return "true" if value else "false"
    return "'" + str(value).replace("'", "''") + "'"


def _render_composite_source(node: _ExpressionNode) -> str:
    """Render grouping, unary, and binary expression nodes."""
    kind = _node_text(node, 0)
    if kind == "not":
        return f"!({_expression_source(_node_child(node, 1))})"
    if kind == "group":
        return f"({_expression_source(_node_child(node, 1))})"
    operator = {
        "and": "&&",
        "or": "||",
        "equal": "==",
        "not-equal": "!=",
    }.get(kind)
    if operator is None:
        raise AssertionError
    left = _expression_source(_node_child(node, 1))
    right = _expression_source(_node_child(node, 2))
    return f"({left} {operator} {right})"


def _node_text(node: _ExpressionNode, index: int) -> str:
    """Read one generated string tag or key from its tuple node."""
    value = node[index]
    if not isinstance(value, str):
        raise TypeError
    return value


def _node_child(node: _ExpressionNode, index: int) -> _ExpressionNode:
    """Read one nested generated node from its tuple parent."""
    value = node[index]
    if not isinstance(value, tuple):
        raise TypeError
    return value


class _ReferenceComparisonError(Exception):
    """Mark an expression comparison outside the test oracle's subset."""


def _reference_truthy(value: object) -> bool:
    """Apply the generated subset's Actions truthiness independently."""
    match value:
        case bool():
            return bool(value)
        case str():
            return bool(value)
        case _:
            raise TypeError


def _reference_expression(
    node: _ExpressionNode,
    contexts: _ExpressionContexts,
    *,
    job_level: bool,
) -> object:
    """Evaluate generated nodes directly, without using the production parser."""
    is_leaf, value = _reference_leaf(node, contexts, job_level=job_level)
    return (
        value if is_leaf else _reference_composite(node, contexts, job_level=job_level)
    )


def _reference_leaf(
    node: _ExpressionNode,
    contexts: _ExpressionContexts,
    *,
    job_level: bool,
) -> tuple[bool, object]:
    """Resolve literal and context leaf nodes independently of the parser."""
    kind = _node_text(node, 0)
    if kind == "literal":
        return True, node[1]
    if kind in {"input", "need", "github-event", "dry-run"}:
        return True, _reference_path(kind, node, contexts)
    if kind == "status":
        return True, _reference_status(
            _node_text(node, 1), contexts, job_level=job_level
        )
    if kind == "from-json":
        return True, bool(node[1])
    return False, None


def _reference_path(
    kind: str, node: _ExpressionNode, contexts: _ExpressionContexts
) -> object:
    """Read one generated path from its varied context."""
    if kind == "input":
        return _context_path(contexts, ("inputs", _node_text(node, 1)))
    if kind == "need":
        return _context_path(contexts, ("needs", _node_text(node, 1), "result"))
    paths = {
        "github-event": ("github", "event_name"),
        "dry-run": ("steps", "release_modes", "outputs", "dry-run"),
    }
    path = paths.get(kind)
    if path is None:
        raise AssertionError
    return _context_path(contexts, path)


def _context_path(contexts: _ExpressionContexts, path: tuple[str, ...]) -> object:
    """Resolve a known generated path through nested context mappings."""
    value: object = contexts
    for key in path:
        if not isinstance(value, dict):
            raise TypeError
        value = value[key]
    return value


def _context_mapping(
    contexts: _ExpressionContexts, path: tuple[str, ...]
) -> dict[str, object]:
    """Read a nested mapping while retaining string-keyed static types."""
    value = _context_path(contexts, path)
    if not isinstance(value, cabc.Mapping):
        raise TypeError
    mapping: dict[str, object] = {}
    for key in value:
        if not isinstance(key, str):
            raise TypeError
        mapping[key] = value[key]
    return mapping


def _context_boolean(contexts: _ExpressionContexts, path: tuple[str, ...]) -> bool:
    """Read a generated boolean status value."""
    value = _context_path(contexts, path)
    if not isinstance(value, bool):
        raise TypeError
    return value


def _reference_status(
    name: str, contexts: _ExpressionContexts, *, job_level: bool
) -> bool:
    """Resolve status functions with the job dependency model."""
    if name == "always":
        return True
    if job_level and name in {"success", "failure"}:
        needs = _context_mapping(contexts, ("needs",))
        results = (_context_path(contexts, ("needs", key, "result")) for key in needs)
        return (
            all(result == "success" for result in results)
            if name == "success"
            else any(result == "failure" for result in results)
        )
    return _context_boolean(contexts, ("status", name))


def _reference_composite(
    node: _ExpressionNode,
    contexts: _ExpressionContexts,
    *,
    job_level: bool,
) -> object:
    """Evaluate generated grouping and operator nodes directly."""
    kind = _node_text(node, 0)
    if kind in {"not", "group"}:
        value = _reference_expression(
            _node_child(node, 1), contexts, job_level=job_level
        )
        return not _reference_truthy(value) if kind == "not" else value
    left = _reference_expression(_node_child(node, 1), contexts, job_level=job_level)
    right = _reference_expression(_node_child(node, 2), contexts, job_level=job_level)
    if kind in {"and", "or"}:
        return _reference_logical(kind, left, right)
    if kind in {"equal", "not-equal"}:
        equal = _reference_equal(left, right)
        return equal if kind == "equal" else not equal
    raise AssertionError


def _reference_logical(kind: str, left: object, right: object) -> object:
    """Select the original operand according to Actions truthiness."""
    if kind == "and":
        return right if _reference_truthy(left) else left
    return left if _reference_truthy(left) else right


def _reference_equal(left: object, right: object) -> bool:
    """Compare generated string or boolean operands with Actions semantics."""
    if type(left) is str and type(right) is str:
        return left.casefold() == right.casefold()
    if type(left) is bool and type(right) is bool:
        return left is right
    raise _ReferenceComparisonError


def _contains_status_function(node: _ExpressionNode) -> bool:
    """Find explicit status functions in a generated tree."""
    return _node_text(node, 0) == "status" or any(
        isinstance(child, tuple) and _contains_status_function(tuple(child))
        for child in node[1:]
    )


@given(case=_expression_cases())
def test_generated_expressions_match_an_independent_reference(
    case: tuple[_ExpressionNode, _ExpressionContexts, bool, bool],
) -> None:
    """Compare composed expressions with a direct model over varied contexts."""
    node, contexts, job_level, wrapped = case
    source = _expression_source(node)
    expression = chr(36) + "{{ " + source + " }}" if wrapped else source
    try:
        expected = _reference_expression(node, contexts, job_level=job_level)
    except _ReferenceComparisonError:
        with pytest.raises(UnsupportedExpressionError) as caught:
            evaluate_expression(expression, contexts, job_level=job_level)

        assert caught.value.issue is ExpressionIssue.UNSUPPORTED_COMPARISON, (
            "generated mixed-type comparisons should remain fail-closed"
        )
        return

    if job_level:
        expected = _reference_truthy(expected)
        if not _contains_status_function(node):
            expected = (
                _reference_status("success", contexts, job_level=True) and expected
            )
    assert evaluate_expression(expression, contexts, job_level=job_level) == expected, (
        f"expression {expression!r} should resolve to {expected!r}"
    )
