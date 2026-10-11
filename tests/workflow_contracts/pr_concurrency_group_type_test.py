"""Refuse a concurrency group written as anything but a string.

The pull-request workflows in this repository all write ``group`` as a string,
so the file-driven contract in ``pr_concurrency_test`` cannot show that it
refuses a group of any other type. Reading one through ``str()`` was the defect:
a mapping's Python text still contains the estate expression, so it rendered the
same sharing, separation, fallback and workflow-keying properties as the real
group and passed every clause. GitHub requires ``concurrency.group`` to be a
string and would reject the workflow that declared it.

These cases call the declaration contract directly with isolated documents
built here, mirroring ``ci.yml``'s concurrency block and replacing only
``group``. A rejected value must fail that contract, and the estate's own group
must still pass it, so the refusal is the type and not the rendering.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pr_concurrency_test as concurrency_contracts
import pytest

#: The group ``ci.yml`` declares, which every correct workflow repeats.
ESTATE_GROUP: typ.Final = (
    "${{ github.workflow }}-${{ github.event.pull_request.number || github.run_id }}"
)

#: The ``cancel-in-progress`` expression the real block carries; only ``group``
#: differs between the cases below.
CANCEL_EXPRESSION: typ.Final = "${{ github.event_name == 'pull_request' }}"


def _document(group: object) -> concurrency_contracts.Workflows:
    """Return a ``ci.yml`` document declaring ``group`` as its concurrency group.

    Parameters
    ----------
    group
        The value to place under ``concurrency:``; the caller supplies one that
        is not a string to show the contract refusing it.

    Returns
    -------
    concurrency_contracts.Workflows
        One freshly built workflow document, keyed by file name.
    """
    return {
        "ci.yml": {
            "concurrency": {
                "group": group,
                "cancel-in-progress": CANCEL_EXPRESSION,
            }
        }
    }


@pytest.mark.parametrize(
    "group",
    [
        {"x": ESTATE_GROUP},
        [ESTATE_GROUP],
        7,
        None,
        False,
    ],
    ids=["mapping", "list", "integer", "none", "false"],
)
def test_a_non_string_group_fails_the_declaration_contract(group: object) -> None:
    """A group of any other type declares no group at all.

    ``False`` and ``None`` are the sharp cases: stringifying either yields text
    that names no expression, so nothing downstream reads the value as a group
    either way, and only the declaration contract can refuse them.
    """
    with pytest.raises(AssertionError, match="declare no group"):
        concurrency_contracts.test_every_pull_request_workflow_declares_a_group(
            _document(group)
        )


def test_the_estate_group_passes_the_declaration_contract() -> None:
    """The estate's own string group satisfies the contract unchanged.

    A contract that refused every document would pass the cases above without
    detecting anything, so this identity case pins the boundary at the type.
    """
    concurrency_contracts.test_every_pull_request_workflow_declares_a_group(
        _document(ESTATE_GROUP)
    )
