"""Property tests for the pull-request lane's closure over workflow calls.

`reachable_workflows` is compared with an independent reference: a
fixed-point iteration that widens the reached set until it stops growing,
sharing nothing with the implementation's work list. The generated graphs are
bounded and finite, and mix multiple roots, branches, converging edges,
cycles, self-calls, duplicate calls and disconnected workflows, which no fixed
example shows at once.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
from hypothesis import given
from hypothesis import strategies as st
from workflow_call_closure import UnresolvedWorkflowCallError, reachable_workflows

if typ.TYPE_CHECKING:
    import collections.abc as cabc

NAMES = [f"w{index}.yml" for index in range(8)]


def _document(calls: cabc.Iterable[str]) -> dict[str, object]:
    """Build a workflow document calling each named local workflow."""
    return {
        "jobs": {
            f"call{index}": {"uses": f"./.github/workflows/{name}"}
            for index, name in enumerate(calls)
        }
    }


@st.composite
def call_graphs(
    draw: st.DrawFn,
) -> tuple[dict[str, list[str]], list[str]]:
    """Draw a closed call graph over a subset of names, and its entries."""
    nodes = draw(st.lists(st.sampled_from(NAMES), min_size=1, max_size=8, unique=True))
    calls = {node: draw(st.lists(st.sampled_from(nodes), max_size=4)) for node in nodes}
    entries = draw(st.lists(st.sampled_from(nodes), max_size=4))
    return calls, entries


def _fixed_point(calls: dict[str, list[str]], entries: list[str]) -> frozenset[str]:
    """Widen the entry set by one call step until it stops growing."""
    reached = frozenset(entries)
    while True:
        widened = reached.union(*(calls[node] for node in reached))
        if widened == reached:
            return reached
        reached = widened


@given(call_graphs())
def test_the_closure_matches_a_fixed_point(
    graph: tuple[dict[str, list[str]], list[str]],
) -> None:
    """Reach exactly what the fixed-point reference reaches, cycles included."""
    calls, entries = graph
    documents = {name: _document(called) for name, called in calls.items()}
    assert reachable_workflows(documents, entries) == _fixed_point(calls, entries), (
        f"the closure disagrees with the reference for {calls!r} from {entries!r}"
    )


@given(call_graphs())
def test_a_dangling_reachable_call_is_refused(
    graph: tuple[dict[str, list[str]], list[str]],
) -> None:
    """Refuse a reachable call to a workflow the reading does not hold."""
    calls, _ = graph
    root = next(iter(calls))
    documents = {name: _document(called) for name, called in calls.items()}
    documents[root] = _document([*calls[root], "gone.yml"])
    with pytest.raises(UnresolvedWorkflowCallError, match=r"gone\.yml"):
        reachable_workflows(documents, [root])
