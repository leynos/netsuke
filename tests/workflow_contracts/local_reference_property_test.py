"""Check local-reference closure against generated finite script graphs.

The generated graphs include a reachable-or-not cycle, duplicate edges and a
disconnected item. Each graph is also scanned with reversed reference order to
ensure traversal order cannot change the covered set.

Run via ``make test-workflow-contracts``.
"""

import tempfile
from pathlib import Path

from hypothesis import given
from hypothesis import strategies as st
from local_reference_inventory import discover_local_items
from local_reference_test_support import create_synthetic_workspace
from local_references import covered_items, root_source_texts

type ScriptGraph = tuple[dict[int, list[int]], list[int]]


@st.composite
def script_graphs(draw: st.DrawFn) -> ScriptGraph:
    """Generate finite graphs with a cycle and a disconnected script."""
    node_count = draw(st.integers(min_value=4, max_value=8))
    active_nodes = range(node_count - 1)
    edges = {
        node: draw(
            st.lists(st.integers(min_value=0, max_value=node_count - 2), max_size=8)
        )
        for node in active_nodes
    }
    edges[0] = [1, 1, *edges[0]]
    edges[1] = [0, *edges[1]]
    roots = draw(
        st.lists(st.integers(min_value=0, max_value=node_count - 2), max_size=6)
    )
    edges[node_count - 1] = []
    return edges, roots


def _reference_reachability(graph: ScriptGraph) -> frozenset[int]:
    """Walk outward from roots with a set and stack independent of production."""
    edges, roots = graph
    reached: set[int] = set()
    pending = list(roots)
    while pending:
        node = pending.pop()
        if node in reached:
            continue
        reached.add(node)
        pending.extend(edges[node])
    return frozenset(reached)


def _scan_graph(
    root: Path,
    graph: ScriptGraph,
    *,
    reverse: bool,
) -> frozenset[str]:
    """Write and scan one ordering of a synthetic script-reference graph."""
    edges, roots = graph
    root_order = list(reversed(roots)) if reverse else roots
    makefile = "all:\n" + "".join(
        f"\t@./scripts/node-{node}.sh\n" for node in root_order
    )
    scripts = {}
    for node, neighbours in edges.items():
        ordered_neighbours = list(reversed(neighbours)) if reverse else neighbours
        scripts[f"scripts/node-{node}.sh"] = "".join(
            f"source node-{neighbour}.sh\n" for neighbour in ordered_neighbours
        )
    create_synthetic_workspace(root, "name: Synthetic\njobs: {}\n", makefile, scripts)
    inventory = discover_local_items(root)
    return covered_items(inventory, root_source_texts(root))


@given(script_graphs())
def test_coverage_matches_oracle_independent_of_reference_order(
    graph: ScriptGraph,
) -> None:
    """Match graph reachability across cycles, duplicates and shuffled edges."""
    with tempfile.TemporaryDirectory() as temporary_directory:
        parent = Path(temporary_directory)
        forward = _scan_graph(parent / "forward", graph, reverse=False)
        reverse = _scan_graph(parent / "reverse", graph, reverse=True)

    expected = frozenset(
        f"scripts/node-{node}.sh" for node in _reference_reachability(graph)
    )
    assert forward == reverse == expected, (
        f"closure differed from graph reachability for {graph!r}"
    )
