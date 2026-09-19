"""Specify the ``vars.`` scan the workflow contracts rely on.

The scan is what makes the prohibition of undefined repository variables
general rather than a list of input names. These cases pin the two properties
that generality rests on: a reference is found wherever a step writes one, and
the single variable this repository does declare is not reported.

Run via ``make test-workflow-contracts``.
"""

import pytest
from workflow_variable_scan import unbound_variable_references

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
