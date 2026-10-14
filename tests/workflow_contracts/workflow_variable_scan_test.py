"""Specify the ``vars.`` scan the workflow contracts rely on.

The scan is what makes the prohibition of undefined repository variables
general rather than a list of input names. These cases pin the two properties
that generality rests on: a reference is found wherever a step writes one, and
the single variable this repository does declare is not reported.

Run via ``make test-workflow-contracts``.
"""

import pytest
from workflow_variable_scan import (
    expression_references,
    reference_occurrences,
    unbound_variable_references,
)

#: A variable this repository has never declared, spelled as a workflow would
#: write it.
UNDECLARED = "${{ vars.CODESCENE_CLI_SHA256 }}"


def test_a_reference_is_found_in_every_field_a_step_can_hold_one() -> None:
    """Read a ``vars.`` reference wherever a step writes one.

    The repository declares no variables, so any ``vars.`` expression resolves
    to the empty string. A scan limited to the ``with`` block would miss the
    ``if`` that decides whether the step runs at all, and the ``env`` entry a
    credential is bound to.
    """
    for field, step in [
        ("if", {"if": UNDECLARED}),
        ("env", {"env": {"X": UNDECLARED}}),
        ("with", {"with": {"x": UNDECLARED}}),
    ]:
        assert unbound_variable_references(step) == [UNDECLARED], (
            f"a reference in `{field}` must be found"
        )


@pytest.mark.parametrize(
    ("label", "expression"),
    [
        # A condition that names a variable in its second operand. The empty
        # value collapses the whole conjunction, so the step silently stops
        # running exactly as it would from a bare reference.
        ("conjunction", "${{ github.event_name == 'push' && vars.SECRET != '' }}"),
        # A function argument, where the variable is not adjacent to `${{`.
        ("function call", "${{ contains(vars.FOO, 'x') }}"),
        # A reference in the second of two expressions in one value.
        ("later expression", "${{ github.ref }} then ${{ vars.NOPE }}"),
    ],
)
def test_a_reference_is_found_anywhere_inside_an_expression(
    label: str, expression: str
) -> None:
    """Read a reference that does not sit immediately after the delimiter.

    An earlier scan matched only the position right after ``${{``, so every
    expression here reported clean while resolving to the empty string — the
    exact failure the scan exists to prevent, in the shape most likely to be
    written by hand.
    """
    assert unbound_variable_references({"if": expression}) == [expression], (
        f"a reference in a {label} must be found"
    )


def test_a_reference_is_found_across_a_newline() -> None:
    """Read a reference in an expression a step breaks across lines.

    A YAML literal block keeps its newlines after parsing, so this is the shape
    a long condition takes once it is wrapped. A scan whose `.` stopped at the
    newline would never close the region and would report the reference as
    absent while the empty value did its work.
    """
    value = "${{ on_push &&\n    vars.SECRET != '' }}"
    assert unbound_variable_references({"run": value}) == [value], (
        "a reference after a newline must be found"
    )


def test_reference_text_outside_an_expression_is_ignored() -> None:
    """Do not report a ``vars.`` spelling that is only literal text.

    The scan locates expression regions before looking for references, so a
    value that merely prints or compares the spelling is not a reference and
    must not be reported as one.
    """
    for literal in ["echo 'vars.FOO'", "assert 'vars.' not in text"]:
        assert not unbound_variable_references({"run": literal}), (
            f"`{literal}` is text, not a reference"
        )


def test_a_value_naming_several_variables_is_reported_once() -> None:
    """Report the value, not each name, when one value reads several.

    The caller reports which values to look at; repeating a value per name
    would read as several faults in a step that has one.
    """
    value = "${{ vars.A != '' || vars.B != '' }}"
    assert unbound_variable_references({"if": value}) == [value], (
        "one offending value must be reported once"
    )


def test_the_permitted_variable_is_not_reported() -> None:
    """Leave the one variable this repository defines alone.

    It selects between the two sccache backends, so the workflows that read it
    are correct and must not be reported.
    """
    assert not unbound_variable_references({
        "if": "${{ vars.NETSUKE_SCCACHE_LOCAL_DIR == 'true' }}"
    }), "the one variable this repository defines must be permitted"


def test_a_step_with_no_reference_is_clean() -> None:
    """Report nothing for a step that reads no repository variable."""
    assert not unbound_variable_references({
        "name": "Checkout",
        "uses": "actions/checkout@abc123",
        "with": {"persist-credentials": "false"},
    }), "a step reading no repository variable must be reported clean"


@pytest.mark.parametrize(
    ("label", "condition"),
    [
        # The plain comparison, which is how this repository writes a gate.
        ("comparison", "vars.CODESCENE_CLI_SHA256 == 'true'"),
        # A compound condition, where the variable is one operand among several.
        ("conjunction", "github.event_name == 'push' && vars.SECRET != ''"),
        # A condition read as a function argument rather than as a comparison.
        ("function call", "contains(vars.FOO, 'x')"),
    ],
)
def test_a_reference_is_found_in_a_condition_written_without_delimiters(
    label: str, condition: str
) -> None:
    """Read a step's ``if`` whole, because GitHub evaluates it either way.

    A condition is an expression whether or not it carries the ``${{ }}``
    delimiters, so a ``vars.`` reference written without them resolves to the
    empty string exactly as a delimited one does — and the undelimited spelling
    is the one this repository uses. A scan that looked only inside delimiters
    reported every condition here clean while the empty value decided whether
    the step ran at all, and the step's own name is the only clue left behind.
    """
    assert unbound_variable_references({"if": condition}) == [condition], (
        f"a {label} naming an undeclared variable must be reported"
    )


def test_the_undelimited_spelling_of_the_permitted_variable_is_not_reported() -> None:
    """Permit the declared variable however the condition is punctuated.

    The permission is a statement about which variable is read, not about
    whether the author wrote the delimiters, so the undelimited spelling has to
    be permitted exactly as the delimited one is. Without this case the fix for
    the undelimited form could be a scan that reports every ``if`` it is handed.
    """
    assert not unbound_variable_references({
        "if": "vars.NETSUKE_SCCACHE_LOCAL_DIR == 'true'"
    }), "the one variable this repository defines must be permitted either way"


@pytest.mark.parametrize(
    ("label", "step"),
    [
        # A run block is text: the dotted pair here is a command and a
        # subcommand, not a reference to a context value.
        ("run block", {"run": "cargo fmt.version --check"}),
        # A message that happens to spell a dotted pair names nothing either.
        ("literal message", {"name": "Bump to version 1.2.3"}),
        # An `env` entry is a value, not a condition, so it is read for
        # delimiters only — as it was before the condition was read whole.
        ("env entry", {"env": {"SPEC": "vars.UNDECLARED"}}),
    ],
)
def test_only_a_condition_is_read_as_a_bare_expression(
    label: str, step: dict[str, object]
) -> None:
    """Keep the whole-value reading to the one field GitHub evaluates as one.

    Reading every string as an expression would report ordinary text — a run
    block naming a subcommand, a release message — as an undeclared variable,
    and a scan that accuses correct workflows is one its readers learn to
    ignore. Only a step's ``if`` is an expression in its own right.
    """
    assert not unbound_variable_references(step), (
        f"a {label} is not an expression and must not be read as one"
    )


def test_an_identifier_is_read_with_its_namespace() -> None:
    """Pair each name with the namespace it is addressed through.

    The pair is what a caller compares against a name it expects. A caller
    asking whether a value *contains* `CS_ACCESS_TOKEN` is asking about the
    text, and `env.NOT_CS_ACCESS_TOKEN` contains it; asking whether some
    identifier's name *is* the credential is the question about the reference.
    """
    assert expression_references("${{ env.CS_ACCESS_TOKEN }}") == [
        ("env", "CS_ACCESS_TOKEN")
    ], "a reference must report its namespace and its name"


@pytest.mark.parametrize(
    ("label", "value"),
    [
        # GitHub's expression grammar delimits strings with single quotes and
        # gives no meaning to double quotes, so only the first is a literal.
        ("single-quoted", "${{ 'env.CS_ACCESS_TOKEN' != '' }}"),
        ("quoted operand", "${{ 'x' == 'secrets.TOKEN' }}"),
    ],
)
def test_an_identifier_inside_a_string_literal_is_not_a_reference(
    label: str, value: str
) -> None:
    """Read a quoted name as text rather than as an address.

    GitHub treats a name as a reference only when it is written unquoted, so
    `${{ 'env.CS_ACCESS_TOKEN' != '' }}` compares a non-empty string literal
    against the empty string: always true, naming no variable, gating nothing.
    A caller holding a step to an exact reference has to see that difference,
    or a gate spelled that way satisfies it while gating nothing at all.
    """
    assert not expression_references(value), (
        f"an identifier inside a {label} string names nothing"
    )


def test_an_identifier_is_read_outside_a_string_literal() -> None:
    """Keep reading the reference a literal sits beside.

    Stripping quoted text is not the same as refusing a value that holds any.
    An expression may compare a literal with a real reference, and the
    reference is still the one the step is gated on.
    """
    assert expression_references("${{ env.CS_ACCESS_TOKEN != 'x' }}") == [
        ("env", "CS_ACCESS_TOKEN")
    ], "a real reference beside a literal must still be read"


@pytest.mark.parametrize(
    ("label", "value"),
    [
        ("no inner space", "${{ vars['CODESCENE_CLI_SHA256'] }}"),
        ("inner spaces", "${{ vars[ 'CODESCENE_CLI_SHA256' ] }}"),
        ("in a condition", "${{ vars['CODESCENE_CLI_SHA256'] != '' }}"),
        (
            "in a function call",
            "${{ contains(vars['CODESCENE_CLI_SHA256'], 'x') }}",
        ),
    ],
)
def test_an_index_reference_is_read_as_a_reference(label: str, value: str) -> None:
    """Read the index syntax as the reference it is.

    GitHub's contexts reference gives an expression two ways to address a value:
    property de-reference, `vars.FOO`, and index, `vars['FOO']`. They name the
    same variable, so an undeclared one resolves to the empty string either way
    — and a scan that read only the dotted form reported this spelling clean
    while the empty value did its work. That is the failure this module exists
    to prevent, arriving in the spelling a `.`-only pattern cannot see.
    """
    assert unbound_variable_references({"if": value}) == [value], (
        f"an index reference ({label}) must be reported"
    )


def test_the_index_spelling_of_the_permitted_variable_is_not_reported() -> None:
    """Permit the declared variable however it is addressed.

    The permission is a statement about which variable is read, not about the
    punctuation used to reach it, so the bracketed spelling has to be permitted
    exactly as the dotted one is.
    """
    assert not unbound_variable_references({
        "if": "${{ vars['NETSUKE_SCCACHE_LOCAL_DIR'] == 'true' }}"
    }), "the one variable this repository defines must be permitted in either syntax"


def test_an_index_reference_is_read_with_its_namespace() -> None:
    """Pair an index reference with the namespace before its bracket.

    The namespace is not inside the brackets, so it has to be read from the
    identifier to their left — the word the index is applied to. Reading the
    quoted name alone would lose which context the step addressed, and a caller
    holding a step to a credential in `env` would then accept the same name
    reached through `secrets`.
    """
    assert expression_references("${{ env['CS_ACCESS_TOKEN'] }}") == [
        ("env", "CS_ACCESS_TOKEN")
    ], "an index reference must report the namespace it is addressed through"


@pytest.mark.parametrize(
    ("label", "value"),
    [
        # A quoted run that is not an index's delimiters names nothing. The
        # quotes here are the string literal's, and the name inside is text.
        ("quoted index spelling", "${{ 'env[\\'CS_ACCESS_TOKEN\\']' != '' }}"),
        # An index whose bracket holds no quoted name states no name to compare
        # against a declared one: `vars[name]` is addressed by a value only the
        # runner can reach.
        ("non-literal index", "${{ vars[name] }}"),
        # The empty name, which would otherwise be read as a reference named ''.
        ("empty index name", "${{ vars[''] }}"),
    ],
)
def test_text_that_addresses_no_name_is_not_a_reference(label: str, value: str) -> None:
    """Decline the spellings that name nothing, and say which they are.

    Each of these sits close enough to a reference to be read as one by a
    looser pattern, and each resolves to nothing that could be held against a
    declared name — so reporting them would be a false accusation, and reading
    them as references would let a caller believe a gate addressed a value the
    condition never mentions.
    """
    assert not expression_references(value), f"a {label} names nothing"


def test_an_occurrence_reports_where_the_reference_was_read() -> None:
    """Report each reference's span into the value it came from.

    A caller asking about the text beside a reference — is this the comparison
    the lane means — has to slice the value it passed, so the offsets must be
    into that value rather than into the body of the region the reference was
    found in. An expression's delimiters and the text before them would
    otherwise offset every position by a fixed amount, and a caller reading its
    own slices would be reading the wrong characters by exactly that much.
    """
    value = "${{ env.CS_ACCESS_TOKEN != '' }}"
    assert reference_occurrences(value) == [
        ("env", "CS_ACCESS_TOKEN", value.index("env"), value.index("env") + 19)
    ], "an occurrence must locate its reference within the value scanned"
