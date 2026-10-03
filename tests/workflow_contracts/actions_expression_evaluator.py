"""Evaluate the bounded GitHub Actions expression subset used by contracts.

The release-path contract resolves workflow guards and input expressions from
parsed YAML. This parser refuses syntax it cannot model so an unknown guard
never silently counts as a skipped step.
"""

import collections.abc as cabc
import enum
import re
import typing as typ


class ExpressionIssue(enum.StrEnum):
    """Classify syntax and context failures in bounded workflow expressions."""

    UNTERMINATED_WRAPPER = "unterminated expression wrapper"
    MIXED_WRAPPER = "expression wrapper must enclose the whole value"
    UNPARSABLE_SYNTAX = "unparsable expression syntax"
    UNEXPECTED_TRAILING_INPUT = "unexpected trailing expression input"
    EXPECTED_VALUE = "expected a value"
    FROM_JSON_SUBSET = "fromJSON only models the strings 'true' and 'false'"
    UNKNOWN_FUNCTION = "unknown function"
    UNHANDLED_STATUS = "unhandled status function"
    DOTTED_PATH_NAME = "a dotted path needs a name"
    BRACKET_PATH_KEY = "a bracketed path needs a quoted key"
    NEEDS_MAPPING = "job-level status checks need a needs mapping"
    NEEDS_ENTRY_MAPPING = "needs entries must be mappings"
    NEEDS_RESULT = "needs entry result is unresolved"
    MISSING_VALUE = "expression ended before a value"
    EXPECTED_SYNTAX = "expected expression syntax"
    UNKNOWN_PATH = "unknown path"
    UNSUPPORTED_TRUTHINESS = "truthiness is not modelled"
    UNSUPPORTED_COMPARISON = "comparison is only modelled for strings and booleans"
    EXPECTED_EXPRESSION_STRING = "expected an expression string"
    INVALID_ARTIFACT_NAME = "artifact name must be a string"
    INCOMPLETE_ARTIFACT_EXPRESSION = "artifact name has an incomplete expression"
    INVALID_ARTIFACT_VALUE = "artifact expression did not resolve to a scalar"


class UnsupportedExpressionError(ValueError):
    """Raised when an expression leaves the grammar the evaluator models.

    An evaluator that answered False outside its grammar would let a guard it
    cannot read pass as one that never runs, so it refuses instead.
    """

    def __init__(self, issue: ExpressionIssue, detail: object | None = None) -> None:
        """Record the failure class and optional expression detail."""
        self.issue = issue
        self.detail = detail
        super().__init__(issue, detail)

    def __str__(self) -> str:
        """Render a stable failure class with optional diagnostic detail."""
        if self.detail is None:
            return self.issue.value
        return f"{self.issue.value}: {self.detail}"


_TOKEN = re.compile(
    r"(?P<space>\s+)|(?P<operator>\$\{\{|\}\}|&&|\|\||==|!=|[!().,\[\]])|"
    r"(?P<string>'(?:''|[^'])*')|(?P<name>[A-Za-z_][A-Za-z0-9_-]*)"
)
_STATUS_FUNCTIONS = frozenset({"always", "cancelled", "success", "failure"})
_MISSING = object()


def _strip_expression_wrapper(expression: str) -> str:
    """Remove one complete Actions expression wrapper, when present."""
    value = expression.strip()
    if value.startswith("${{"):
        if not value.endswith("}}"):
            raise UnsupportedExpressionError(ExpressionIssue.UNTERMINATED_WRAPPER)
        return value[3:-2].strip()
    if "${{" in value or "}}" in value:
        raise UnsupportedExpressionError(ExpressionIssue.MIXED_WRAPPER)
    return value


def _tokenize(expression: str) -> list[tuple[str, str]]:
    """Tokenize a bounded Actions expression or refuse its first unknown token."""
    tokens: list[tuple[str, str]] = []
    position = 0
    while position < len(expression):
        match = _TOKEN.match(expression, position)
        if match is None:
            raise UnsupportedExpressionError(
                ExpressionIssue.UNPARSABLE_SYNTAX,
                f"offset {position}: {expression[position : position + 12]!r}",
            )
        position = match.end()
        if match.lastgroup != "space":
            tokens.append((typ.cast("str", match.lastgroup), match.group()))
    return tokens


class _ExpressionParser:
    """Parse and evaluate the expression subset used by release workflows."""

    def __init__(
        self,
        expression: str,
        contexts: cabc.Mapping[str, object],
        *,
        job_level: bool,
    ) -> None:
        """Initialize parser state for one workflow expression."""
        self.tokens = _tokenize(_strip_expression_wrapper(expression))
        self.contexts = contexts
        self.job_level = job_level
        self.position = 0
        self.has_status_function = False

    def evaluate(self) -> object:
        """Evaluate all tokens and apply GitHub's job-level default check."""
        value = self._parse_or()
        if self.position != len(self.tokens):
            raise UnsupportedExpressionError(
                ExpressionIssue.UNEXPECTED_TRAILING_INPUT, self._peek_value()
            )
        if not self.job_level:
            return value
        should_run = _truthy(value)
        return (
            self._job_success() and should_run
            if not self.has_status_function
            else should_run
        )

    def _parse_or(self) -> object:
        """Evaluate disjunctions from left to right."""
        value = self._parse_and()
        while self._accept("||"):
            right = self._parse_and()
            value = value if _truthy(value) else right
        return value

    def _parse_and(self) -> object:
        """Evaluate conjunctions from left to right."""
        value = self._parse_comparison()
        while self._accept("&&"):
            right = self._parse_comparison()
            value = right if _truthy(value) else value
        return value

    def _parse_comparison(self) -> object:
        """Evaluate an optional equality or inequality comparison."""
        left = self._parse_unary()
        if self._peek_value() not in {"==", "!="}:
            return left
        operator = self._take()[1]
        right = self._parse_unary()
        equal = _equal(left, right)
        return equal if operator == "==" else not equal

    def _parse_unary(self) -> object:
        """Evaluate recursive logical negation before a primary value."""
        if self._accept("!"):
            return not _truthy(self._parse_unary())
        return self._parse_primary()

    def _parse_primary(self) -> object:
        """Parse a literal, grouped expression, function call, or path."""
        kind, value = self._take()
        if value == "(":
            nested = self._parse_or()
            self._expect(")")
            return nested
        if kind == "string":
            return value[1:-1].replace("''", "'")
        if kind != "name":
            raise UnsupportedExpressionError(ExpressionIssue.EXPECTED_VALUE, value)
        if value.lower() in {"true", "false"}:
            return value.lower() == "true"
        if self._accept("("):
            return self._call(value)
        return self._resolve_path(value)

    def _call(self, name: str) -> object:
        """Evaluate one supported function call and its arguments."""
        if name == "fromJSON":
            argument = self._parse_or()
            self._expect(")")
            if not isinstance(argument, str) or argument not in {"true", "false"}:
                raise UnsupportedExpressionError(ExpressionIssue.FROM_JSON_SUBSET)
            return argument == "true"
        if name not in _STATUS_FUNCTIONS:
            raise UnsupportedExpressionError(ExpressionIssue.UNKNOWN_FUNCTION, name)
        self._expect(")")
        self.has_status_function = True
        match name:
            case "always":
                return True
            case "success" if self.job_level:
                return self._job_success()
            case "failure" if self.job_level:
                return self._job_failure()
            case "cancelled" | "failure" | "success":
                return _resolve_path(self.contexts, ("status", name))
        raise UnsupportedExpressionError(ExpressionIssue.UNHANDLED_STATUS, name)

    def _resolve_path(self, root: str) -> object:
        """Resolve a root name with its dotted or bracketed path components."""
        parts = [root]
        while True:
            if self._accept("."):
                kind, key = self._take()
                if kind != "name":
                    raise UnsupportedExpressionError(ExpressionIssue.DOTTED_PATH_NAME)
                parts.append(key)
            elif self._accept("["):
                kind, key = self._take()
                if kind != "string":
                    raise UnsupportedExpressionError(ExpressionIssue.BRACKET_PATH_KEY)
                self._expect("]")
                parts.append(key[1:-1].replace("''", "'"))
            else:
                break
        return _resolve_path(self.contexts, tuple(parts))

    def _job_success(self) -> bool:
        """Require every dependency in the job's needs context to succeed."""
        return all(result == "success" for result in self._need_results())

    def _job_failure(self) -> bool:
        """Report whether a dependency in the job's needs context failed."""
        return any(result == "failure" for result in self._need_results())

    def _need_results(self) -> list[str]:
        """Return every dependency result for job-level status checks."""
        needs = self.contexts.get("needs", _MISSING)
        if not isinstance(needs, cabc.Mapping):
            raise UnsupportedExpressionError(ExpressionIssue.NEEDS_MAPPING)
        results: list[str] = []
        for name, dependency in needs.items():
            if not isinstance(dependency, cabc.Mapping):
                raise UnsupportedExpressionError(
                    ExpressionIssue.NEEDS_ENTRY_MAPPING, name
                )
            result = dependency.get("result", _MISSING)
            if not isinstance(result, str):
                raise UnsupportedExpressionError(ExpressionIssue.NEEDS_RESULT, name)
            results.append(result)
        return results

    def _peek_value(self) -> str | None:
        """Return the next token without consuming it."""
        return (
            self.tokens[self.position][1] if self.position < len(self.tokens) else None
        )

    def _take(self) -> tuple[str, str]:
        """Consume the next token or report an incomplete expression."""
        if self.position == len(self.tokens):
            raise UnsupportedExpressionError(ExpressionIssue.MISSING_VALUE)
        token = self.tokens[self.position]
        self.position += 1
        return token

    def _accept(self, value: str) -> bool:
        """Consume the next token only when its value matches."""
        if self._peek_value() != value:
            return False
        self.position += 1
        return True

    def _expect(self, value: str) -> None:
        """Consume one required punctuation token."""
        if not self._accept(value):
            raise UnsupportedExpressionError(
                ExpressionIssue.EXPECTED_SYNTAX,
                f"{value!r}; got {self._peek_value()!r}",
            )


def _resolve_path(
    contexts: cabc.Mapping[str, object], parts: tuple[str, ...]
) -> object:
    """Resolve every path component, refusing missing or non-mapping parents."""
    value: object = contexts
    for part in parts:
        if not isinstance(value, cabc.Mapping) or part not in value:
            raise UnsupportedExpressionError(
                ExpressionIssue.UNKNOWN_PATH, ".".join(parts)
            )
        value = value[part]
    return value


def _truthy(value: object) -> bool:
    """Apply Actions conditional truthiness, including non-empty string values."""
    match value:
        case bool() as boolean:
            return boolean
        case str() as string:
            return bool(string)
        case _:
            raise UnsupportedExpressionError(
                ExpressionIssue.UNSUPPORTED_TRUTHINESS, type(value).__name__
            )


def _equal(left: object, right: object) -> bool:
    """Compare supported strings or booleans, refusing coercions."""
    match left, right:
        case str() as first, str() as second:
            return first.casefold() == second.casefold()
        case bool() as first, bool() as second:
            return first is second
        case _:
            raise UnsupportedExpressionError(ExpressionIssue.UNSUPPORTED_COMPARISON)


def evaluate_expression(
    expression: str,
    contexts: cabc.Mapping[str, object],
    *,
    job_level: bool = False,
) -> object:
    """Evaluate a bounded workflow expression, refusing syntax it cannot model.

    Job guards receive GitHub's implicit ``success()`` check unless the
    expression contains a status-check function. A job-level ``success()``
    requires every entry in ``needs`` to report ``success``.

    Parameters
    ----------
    expression
        A workflow expression, with or without the ``${{ }}`` wrapper.
    contexts
        The context mappings referenced by the expression.
    job_level
        Whether to apply job-guard status-check semantics.

    Returns
    -------
    object
        The evaluated value, or a boolean when ``job_level`` is true.

    Examples
    --------
    >>> evaluate_expression("${{ fromJSON('true') && !false }}", {})
    True
    >>> evaluate_expression("'false'", {})
    'false'
    """
    return _ExpressionParser(expression, contexts, job_level=job_level).evaluate()
