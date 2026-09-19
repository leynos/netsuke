"""Specify the ``vars.`` scan the workflow contracts rely on.

The scan is what makes the prohibition of undefined repository variables
general rather than a list of input names. These cases pin the two properties
that generality rests on: a reference is found wherever a step writes one, and
the single variable this repository does declare is not reported.

Run via ``make test-workflow-contracts``.
"""

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
