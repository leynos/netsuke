"""Properties for the ``vars.`` scan, over generated expressions.

The example-based suite in ``workflow_variable_scan_test`` states one spelling at
a time what an expression reads as. These properties state the same contracts
over expressions built from the grammar, using ``variable_reference_forms`` as
the oracle: the model records what it named when it wrote the text, so a
disagreement is between the scan and an independent account of the expression
rather than between two readings of it.

Every property states its case about *one* spelling of an expression at a time.
The two spellings — a region inside ``${{ }}``, and a condition written bare —
are read by the same scan through different entry points, and a value that
carried both would have its references counted twice by design, since the
whole-value reading is asked for in addition to the regions. So a generated
value is either a bare condition or a parenthesised region, never both, which is
also the only shape a workflow writes.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

from hypothesis import example, given, settings
from variable_expression_forms import (
    Expression,
    Value,
    conditions,
    permitted_conditions,
    silent_expressions,
    values,
)
from variable_reference_forms import (
    PERMITTED_VARIABLE_NAME,
    VARIABLE_NAMESPACE,
    Fragment,
    dotted,
    indexed,
    literal,
)
from workflow_variable_scan import (
    PERMITTED_VARIABLE_NAME as SCAN_PERMITTED_VARIABLE_NAME,
)
from workflow_variable_scan import VARIABLE_NAMESPACE as SCAN_VARIABLE_NAMESPACE
from workflow_variable_scan import (
    expression_references,
    reference_occurrences,
    unbound_variable_references,
)

#: A value naming a variable this repository has never declared, spelled as a
#: workflow would write it. Used as the example the property is stated over, so
#: the search starts from an input that already names something.
UNDECLARED: typ.Final[str] = "${{ vars.CODESCENE_CLI_SHA256 }}"


def _spellings(expression: Expression) -> tuple[tuple[str, dict[str, bool]], ...]:
    """Return the expression in both spellings, with each one's read options.

    A step's ``if`` is evaluated as an expression whether or not it carries the
    delimiters, so the same body is one contract in two punctuations. Stating
    the pair together is what makes that visible: a scan that read only the
    delimited form would pass one case and fail the other.

    Returns
    -------
    tuple[tuple[str, dict[str, bool]], ...]
        ``(text, read options)`` pairs, the bare spelling first.
    """
    return (
        (expression.render(delimited=False), {"bare": True}),
        (expression.render(), {}),
    )


def test_the_two_namespaces_the_scan_declares_are_the_model_s() -> None:
    """Hold the model's rule vocabulary to the scan module's.

    The model states the offending-reference rule a second time so that the
    properties comparing the two are comparing a reading against a *rule*. That
    is only worth anything while both sides mean the same namespace and the same
    permitted name, and a rename on either side would otherwise leave the model
    quietly permissive — permitting a variable the scan reports, or the reverse
    — with every property still passing.
    """
    assert SCAN_VARIABLE_NAMESPACE == VARIABLE_NAMESPACE, (
        "the model and the scan must address the same namespace"
    )
    assert SCAN_PERMITTED_VARIABLE_NAME == PERMITTED_VARIABLE_NAME, (
        "the model and the scan must permit the same variable"
    )


@settings(max_examples=400, derandomize=True, deadline=None)
@example(expression=Expression.of(dotted("vars", "CODESCENE_CLI_SHA256")))
@example(expression=Expression.of(indexed("vars", "CODESCENE_CLI_SHA256")))
@example(
    expression=Expression.of(
        dotted("github", "event_name"),
        literal("push"),
        dotted("vars", "SECRET"),
    )
)
@example(expression=Expression.of(literal("vars.UNDECLARED")))
@given(expression=conditions())
def test_a_condition_is_read_the_same_in_either_spelling(
    expression: Expression,
) -> None:
    """Read a bare condition and its delimited twin identically.

    GitHub evaluates a step's ``if`` as an expression with or without the
    delimiters, and this repository writes the bare form. The two are one
    contract, so the same body has to yield the same references and the same
    verdict in either punctuation — otherwise the scan is reading the
    punctuation rather than the expression.

    A value is reported *as a whole* rather than per reference, because the
    caller needs to know which values to look at: one value carrying two
    undeclared names is one fault to report, not two.
    """
    expected = [
        (reference.namespace, reference.name) for reference in expression.references
    ]
    undeclared = list(expression.undeclared())

    verdicts = []
    for text, options in _spellings(expression):
        reported = unbound_variable_references({"if": text})
        assert bool(reported) == bool(undeclared), (
            f"{text!r} names {undeclared!r} and must be reported when it does"
        )
        assert not reported or reported == [text], (
            f"{text!r} holds one value, so one report: {reported!r}"
        )
        assert expression_references(text, **options) == expected, (
            f"the references of {text!r}"
        )
        verdicts.append(bool(reported))

    assert verdicts[0] == verdicts[1], (
        f"either spelling of {expression.body!r} must read the same: {verdicts!r}"
    )


@settings(max_examples=300, derandomize=True, deadline=None)
@example(expression=Expression.of(dotted("vars", "A"), dotted("vars", "A")))
@example(expression=Expression.of(indexed("vars", "B"), indexed("vars", "B")))
@given(expression=conditions())
def test_each_reference_is_reported_exactly_once(expression: Expression) -> None:
    """Report one occurrence per reference the expression names, in order.

    A repeated name is two occurrences, not one: a caller asking how often a
    value reads something is served by the count, so a scan that collapsed
    repeats would answer about the wrong expression. The same holds the other
    way — an occurrence reported for text the model never wrote as a reference
    would read as a name the workflow addresses when it does not.

    The span is checked against the text the model rendered rather than against
    the name alone, so the offsets a caller slices a value with are shown to
    cover the reference and nothing else.
    """
    expected = [
        (reference.namespace, reference.name) for reference in expression.references
    ]
    for text, options in _spellings(expression):
        occurrences = reference_occurrences(text, **options)
        pairs = [(namespace, name) for namespace, name, _, _ in occurrences]
        assert pairs == expected, f"the occurrences of {text!r}, in order"
        for (namespace, name, start, end), reference in zip(
            occurrences, expression.references, strict=True
        ):
            assert (namespace, name) == (reference.namespace, reference.name), (
                f"the occurrence at this position must be the reference it "
                f"came from: {namespace}.{name} is drawn where "
                f"{reference.namespace}.{reference.name} is expected, "
                f"in {text!r}"
            )
            assert text[start:end] == reference.rendered, (
                f"the span of {namespace}.{name} in {text!r}"
            )


@settings(max_examples=200, derandomize=True, deadline=None)
@example(expression=Expression.of(dotted("env", "CS_ACCESS_TOKEN")))
@example(expression=Expression.of(dotted("secrets", "NETSUKE_SCCACHE_LOCAL_DIR")))
@given(expression=permitted_conditions())
def test_only_the_declared_variable_is_permitted(expression: Expression) -> None:
    """Report nothing for a condition that names no undeclared variable.

    The permission is a statement about which variable is read, not about the
    scan's caution: the one repository variable that is declared may be read in
    either syntax, under either spelling of the condition, and a name reached
    through a *different* namespace is a different value — `env.X` is not
    `vars.X`, and permitting the second does not permit the first.

    The model's own verdict is asserted alongside, so the property cannot pass
    by generating the permitted variable everywhere: if the model ever names an
    undeclared variable and the scan stays quiet, that is the disagreement. The
    expression is required to name something, so this is not a property about
    the empty string.
    """
    assert expression.references, f"{expression.body!r} must name a reference"

    for text, options in _spellings(expression):
        assert not unbound_variable_references({"if": text}), (
            f"{text!r} declares only {expression.references!r}"
        )
        assert expression_references(text, **options), (
            f"{text!r} does name a reference, so the read is not vacuous"
        )


@settings(max_examples=300, derandomize=True, deadline=None)
@example(value=Value((Expression.of(dotted("vars", "CODESCENE_CLI_SHA256")),)))
@example(
    value=Value((
        "installing",
        Expression.of(dotted("github", "ref")),
        Expression.of(indexed("vars", "CODESCENE_CLI_SHA256")),
    ))
)
@example(value=Value((Expression.of(literal("vars.UNDECLARED")), "and prose")))
@example(value=Value(("cargo fmt.version --check",)))
@given(value=values())
def test_a_value_is_read_for_its_regions_and_its_prose_is_not(value: Value) -> None:
    """Read every region a value holds, and only the regions.

    A step value is text: `run` blocks, messages, and URLs legitimately spell a
    dotted pair — a command and a subcommand, a version, a hostname — and none
    of them addresses a context value. So the regions are located first and the
    scan reads those, which is what makes this property two-sided: every region
    the model wrote is read, and no prose word beside one is read as a
    reference.
    """
    text = value.render()
    undeclared = list(value.undeclared())

    reported = unbound_variable_references({"run": text})
    assert bool(reported) == bool(undeclared), (
        f"{text!r} names {undeclared!r} and must be reported when it does"
    )
    assert not reported or reported == [text], (
        f"{text!r} is one value, so one report: {reported!r}"
    )

    expected = [
        (reference.namespace, reference.name) for reference in value.references()
    ]
    assert expression_references(text) == expected, f"the references of {text!r}"
    assert len(reference_occurrences(text)) == len(value.references()), (
        f"each reference in {text!r} is one occurrence"
    )


@settings(max_examples=200, derandomize=True, deadline=None)
@example(expression=Expression.of(literal("vars.UNDECLARED")))
@example(expression=Expression.of(literal("vars['FOO']")))
@example(expression=Expression.of(Fragment("echo 'vars.FOO'"), Fragment("true")))
@given(expression=silent_expressions())
def test_text_that_names_nothing_is_never_reported(expression: Expression) -> None:
    """Accuse nothing of naming a variable when it names none.

    The scan exists to catch a workflow that reads an undeclared variable, so
    every false report costs it the reader's attention — and a scan that accuses
    correct workflows is one its readers learn to ignore. These inputs are the
    near misses: a quoted literal spelling a reference exactly, an index that
    the quotes make text, and dotted pairs written as ordinary words.

    Both spellings of a step's ``if`` are asserted, because the bare reading is
    the wider one: it scans text that was never inside a region, so it is where
    prose most nearly becomes a reference.
    """
    assert not expression.references, f"{expression.body!r} must name nothing"

    for text, _ in _spellings(expression):
        assert not unbound_variable_references({"if": text}), (
            f"{text!r} names nothing and must not be reported"
        )
        assert not expression_references(text, bare=True), (
            f"{text!r} must read no reference at all"
        )
