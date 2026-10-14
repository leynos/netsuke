"""Property tests for the nextest goal derivation over generated Makefiles.

Each case draws a small goal graph: which goals run `cargo nextest run`
directly, and which reach another goal through a declared prerequisite or a
`$(MAKE)` call. The graph may branch, cycle or fall apart into pieces. The
expected goal set is computed by an independent reverse reachability over the
drawn edges, not by the reader under test.

Run via ``make test-workflow-contracts``.
"""

from hypothesis import given
from hypothesis import strategies as st
from nextest_lane_rules import nextest_goals

GOALS = tuple(f"goal-{index}" for index in range(6))

type Graph = tuple[frozenset[str], frozenset[tuple[str, str, bool]]]


@st.composite
def goal_graphs(draw: st.DrawFn) -> Graph:
    """Draw direct suite goals and (caller, callee, via-sub-make) edges."""
    direct = frozenset(draw(st.sets(st.sampled_from(GOALS), max_size=3)))
    edges = draw(
        st.sets(
            st.tuples(st.sampled_from(GOALS), st.sampled_from(GOALS), st.booleans()),
            max_size=10,
        )
    )
    return direct, frozenset(edges)


def _render(direct: frozenset[str], edges: frozenset[tuple[str, str, bool]]) -> str:
    """Write the graph as a Makefile, one rule per goal."""
    lines = []
    for goal in GOALS:
        needs = sorted(
            callee for caller, callee, sub in edges if caller == goal and not sub
        )
        calls = sorted(
            callee for caller, callee, sub in edges if caller == goal and sub
        )
        lines.append(f"{goal}: {' '.join(needs)}".rstrip())
        lines.extend(f"\t$(MAKE) {callee}" for callee in calls)
        if goal in direct:
            lines.append("\tcargo nextest run --workspace")
    return "\n".join(lines) + "\n"


def _reaching(
    direct: frozenset[str], edges: frozenset[tuple[str, str, bool]]
) -> set[str]:
    """Return every goal with a path to a direct goal, by breadth-first search."""
    found = set(direct)
    frontier = list(direct)
    while frontier:
        target = frontier.pop()
        for caller, callee, _ in edges:
            if callee == target and caller not in found:
                found.add(caller)
                frontier.append(caller)
    return found


@given(goal_graphs())
def test_nextest_goals_match_reverse_reachability(graph: Graph) -> None:
    """The derived goals are exactly those that can reach the suite."""
    direct, edges = graph
    expected = _reaching(direct, edges)
    derived = set(nextest_goals(_render(direct, edges)))
    assert derived == expected, (
        f"derived {sorted(derived)} but {sorted(expected)} reach the suite "
        f"in {_render(direct, edges)!r}"
    )
