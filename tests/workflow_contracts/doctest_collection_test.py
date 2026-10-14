"""The workflow-contract gate collects docstring examples.

``make test-workflow-contracts`` passes ``--doctest-modules``, without
which pytest collects test functions only and every example in the
modules under ``tests/workflow_contracts`` is prose that nothing
executes. Deleting the flag changes no test and breaks no import: the
ordinary suite passes exactly as before, and the examples silently stop
running. Nothing else in the tree would notice, which is why this
module exists.

The assertion is against the recipe `make` runs, not against a mention
of the flag anywhere in the file. A flag named in a comment, in a
variable, or in a neighbouring target is a flag the gate does not pass.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
from makefile_recipes import MakefileTargetError, makefile_recipe

#: The target whose recipe runs this suite.
CONTRACT_TARGET: typ.Final[str] = "test-workflow-contracts"

#: The flag that turns the modules' examples into collected tests.
DOCTEST_FLAG: typ.Final[str] = "--doctest-modules"

#: The paths the recipe collects. The flag only collects examples in
#: modules pytest already walks, so the flag and the path together are
#: what executes anything; the flag beside some other directory would
#: satisfy a laxer reading while collecting none of these examples.
COLLECTED_PATH: typ.Final[str] = "tests/workflow_contracts"


def test_the_contract_gate_collects_docstring_examples() -> None:
    """The recipe passes the flag, over the path that holds the examples.

    Asserted against the recipe rather than the file, because a flag in
    a comment or a neighbouring target is one `make` never passes.

    Proved by mutation: removing ``--doctest-modules`` from the recipe
    fails this test, and so does pointing the run at another directory.
    """
    recipe = makefile_recipe(CONTRACT_TARGET)

    assert DOCTEST_FLAG in recipe, (
        f"the {CONTRACT_TARGET} recipe does not pass {DOCTEST_FLAG}, so "
        f"pytest collects test functions only and every docstring example "
        f"under {COLLECTED_PATH} is prose nothing executes; the suite would "
        f"pass unchanged and the loss would look exactly like success"
    )
    assert COLLECTED_PATH in recipe, (
        f"the {CONTRACT_TARGET} recipe does not run over {COLLECTED_PATH}, so "
        f"{DOCTEST_FLAG} collects none of the examples this contract is about"
    )


def test_an_absent_target_is_refused_rather_than_read_as_unflagged() -> None:
    """A renamed target must fail, not read as a recipe without the flag.

    The two are different faults with different remedies, and folding
    them together would report a rename as a deleted flag and send a
    reader to the wrong line.
    """
    with pytest.raises(MakefileTargetError, match=r"no-such-target"):
        makefile_recipe("no-such-target")


def test_the_recipe_reading_stops_at_the_next_target() -> None:
    """A recipe is its own tab-indented lines and nothing after them.

    Every target in this Makefile is followed by another, so a reading
    that ran on would pick up the next recipe's flags and report them as
    this one's. That is how a contract comes to assert a command it has
    never seen run.
    """
    recipe = makefile_recipe(CONTRACT_TARGET)

    assert recipe, f"the {CONTRACT_TARGET} recipe must not be empty"
    assert all(line.startswith("\t") for line in recipe.splitlines()), (
        "a recipe is tab-indented lines only; anything else means the "
        "reading ran past the target it was asked for"
    )
    assert "test-windows-msi-release-rank" not in recipe, (
        "the reading reached the following target, so this contract would "
        "assert flags that belong to another recipe"
    )
