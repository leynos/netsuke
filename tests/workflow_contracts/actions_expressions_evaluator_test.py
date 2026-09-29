"""Test the bounded Actions expression evaluator's accepted grammar."""

import pytest
from actions_expressions import UnsupportedExpressionError, evaluate_expression

CONTEXTS = {
    "github": {"event": {"action": "opened"}},
    "needs": {
        "metadata": {"result": "success"},
        "windows-native-recipe-smoke": {"result": "skipped"},
    },
    "status": {"cancelled": False, "failure": False, "success": True},
    "steps": {"release_modes": {"outputs": {"dry-run": "true"}}},
}


@pytest.mark.parametrize(
    ("expression", "expected"),
    [
        ("${{ (true && !false) || false }}", True),
        ("'false' && true", True),
        ("false || 'false'", True),
        ("needs.metadata.result == 'success'", True),
        ("needs.metadata.result != 'failure'", True),
        ("fromJSON('true') == true", True),
        ("fromJSON('false') == false", True),
        ("false != true", True),
        ("steps.release_modes.outputs['dry-run'] == 'true'", True),
        ("needs['windows-native-recipe-smoke'].result == 'skipped'", True),
        ("fromJSON(steps.release_modes.outputs['dry-run'])", True),
    ],
    ids=[
        "wrapper-parentheses-and-unary-not",
        "and",
        "or",
        "equal-string",
        "not-equal-string",
        "equal-boolean",
        "false-json-boolean",
        "not-equal-boolean",
        "bracketed-hyphenated-key",
        "bracketed-path-with-dotted-tail",
        "from-json-path",
    ],
)
def test_supported_expression_operators_and_paths(
    expression: str, expected: object
) -> None:
    """Evaluate each supported operator and both path forms."""
    assert evaluate_expression(expression, CONTEXTS) is expected, (
        f"supported expression {expression!r} should resolve to {expected!r}"
    )


@pytest.mark.parametrize(
    ("expression", "expected"),
    [("'false'", True), ("''", False), ("true", True), ("false", False)],
    ids=["non-empty-false-string", "empty-string", "true-literal", "false-literal"],
)
def test_job_conditions_use_actions_truthiness(
    expression: str, expected: object
) -> None:
    """Treat every non-empty string as true, including the string 'false'."""
    assert evaluate_expression(expression, {"needs": {}}, job_level=True) is expected, (
        f"job expression {expression!r} should use Actions truthiness"
    )


def test_job_guard_adds_implicit_success_when_no_status_function_is_present() -> None:
    """Do not run a bare true guard when a needed job was skipped."""
    assert evaluate_expression("true", CONTEXTS, job_level=True) is False, (
        "job-level guards need successful dependencies by default"
    )


def test_explicit_status_function_replaces_the_implicit_success_check() -> None:
    """Allow an explicit cancellation check to handle a skipped dependency."""
    assert evaluate_expression("!cancelled() && true", CONTEXTS, job_level=True), (
        "an explicit status function replaces implicit success"
    )


@pytest.mark.parametrize(
    ("needs", "expected"),
    [
        ({"metadata": {"result": "success"}}, True),
        ({"metadata": {"result": "success"}, "smoke": {"result": "skipped"}}, False),
        ({"metadata": {"result": "failure"}}, False),
    ],
    ids=["all-success", "one-skipped", "one-failed"],
)
def test_job_success_requires_every_dependency_to_succeed(
    needs: dict[str, dict[str, str]], expected: object
) -> None:
    """Require every declared need to have succeeded at job level."""
    assert (
        evaluate_expression("success()", {"needs": needs}, job_level=True) is expected
    ), "success() must require every dependency to succeed"


@pytest.mark.parametrize(
    ("expression", "contexts", "expected"),
    [
        ("always()", {}, True),
        ("cancelled()", {"status": {"cancelled": True}}, True),
        ("failure()", {"status": {"failure": True}}, True),
        ("success()", {"status": {"success": True}}, True),
    ],
    ids=["always", "cancelled", "failure", "success"],
)
def test_step_status_functions(
    expression: str, contexts: dict[str, object], expected: object
) -> None:
    """Evaluate the supported step-level status functions from status context."""
    assert evaluate_expression(expression, contexts) is expected, (
        f"status function {expression!r} should resolve from context"
    )


@pytest.mark.parametrize(
    ("expression", "contexts", "message"),
    [
        ("needs.unknown.result", CONTEXTS, "unknown path"),
        ("mystery()", CONTEXTS, "unknown function"),
        ("true @ false", CONTEXTS, "unparsable expression syntax"),
        ("(true", CONTEXTS, r"expected expression syntax: '\)'"),
        ("fromJSON('yes')", CONTEXTS, "fromJSON only models"),
        ("success()", {}, "needs mapping"),
        (
            "context.value && true",
            {"context": {"value": {}}},
            "truthiness is not modelled",
        ),
    ],
    ids=[
        "unknown-path",
        "unknown-function",
        "unparsable-token",
        "unclosed-parenthesis",
        "unsupported-from-json-value",
        "unresolved-job-needs",
        "unsupported-context-value",
    ],
)
def test_the_evaluator_refuses_unmodelled_expressions(
    expression: str, contexts: dict[str, object], message: str
) -> None:
    """Fail closed when a path, function, value, or expression is unsupported."""
    with pytest.raises(UnsupportedExpressionError, match=message):
        evaluate_expression(expression, contexts, job_level=expression == "success()")
