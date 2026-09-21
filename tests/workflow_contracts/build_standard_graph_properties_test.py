"""Drive the Makefile prerequisite reading over graphs nobody wrote down.

``build_standard_wiring_test`` asks ``gated_targets`` which goals reach the
capability check in this repository's Makefile. One Makefile cannot separate
that reading from several weaker ones: a walk that followed only direct
prerequisites, or that ignored order-only edges, or that dropped a goal the
moment a cycle appeared, agrees with the correct reading on a file whose graph
happens to be shallow and acyclic.

These generate the graph instead. Each case renders a synthetic Makefile from a
known edge set, reads it back, and compares the answer with an independent
reachability model — a worklist closure, rather than the recursive walk under
test, so the two can disagree. The generated shapes include the cases the
repository's own file does not currently carry: disconnected goals, order-only
edges, two goals sharing one path to the check, and cycles.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

from build_standard_predicates import (
    CAPABILITY_TARGET,
    gated_targets,
    rule_prerequisites,
)
from hypothesis import assume, example, given
from hypothesis import strategies as st

#: Goal names a generated file may use, alongside the capability check.
#:
#: `target/debug/$(APP)` is present because that is the shape the repository
#: attaches the check to with an order-only edge, and its punctuation is what a
#: rule-header pattern is most likely to mis-parse.
GOAL_NAMES: typ.Final[tuple[str, ...]] = (
    "build",
    "lint",
    "lint-clippy",
    "test",
    "target/debug/$(APP)",
    "bench-build",
)

#: Every name a generated graph draws on, the capability check included, so a
#: goal may declare it directly.
NODE_NAMES: typ.Final[tuple[str, ...]] = (*GOAL_NAMES, CAPABILITY_TARGET)

#: A graph, as goal to its ordinary and order-only prerequisites.
type Graph = dict[str, tuple[tuple[str, ...], tuple[str, ...]]]


def render(graph: Graph) -> str:
    """Return ``graph`` as Makefile text.

    Recipes and `##` help comments are included because both are what the
    reading has to look past: a recipe line is not a rule header, and the help
    text after `##` is not a prerequisite.

    Returns
    -------
    str
        A Makefile declaring each goal with its prerequisites.
    """
    lines = ["# A generated file.", "SHELL := /bin/sh", ""]
    for goal, (ordinary, order_only) in graph.items():
        header = f"{goal}:"
        if ordinary:
            header += " " + " ".join(ordinary)
        if order_only:
            header += " | " + " ".join(order_only)
        lines.extend((f"{header} ## builds {goal}", "\t@echo building", ""))
    return "\n".join(lines)


def reaches_capability(graph: Graph) -> frozenset[str]:
    """Return the goals from which the capability check is reachable.

    The independent model. Where the predicate under test walks the graph
    recursively from each goal, this closes over the edges from the check
    backwards with a worklist, so a defect in one is not a defect in both.
    Order-only edges are ordinary edges here, which is the claim being made
    about them.

    Returns
    -------
    frozenset[str]
        Every declared goal whose prerequisite chain leads to the capability
        check, and the check itself when the file declares it.
    """
    edges: dict[str, set[str]] = {
        goal: set(ordinary) | set(order_only)
        for goal, (ordinary, order_only) in graph.items()
    }
    # Only declared goals can be reported: a name that appears solely as a
    # prerequisite has no rule of its own, and the reading is over rules.
    reaching = {goal for goal in edges if goal == CAPABILITY_TARGET}
    pending = True
    while pending:
        pending = False
        for goal, prerequisites in edges.items():
            if goal in reaching:
                continue
            if any(
                prerequisite == CAPABILITY_TARGET or prerequisite in reaching
                for prerequisite in prerequisites
            ):
                reaching.add(goal)
                pending = True
    return frozenset(reaching)


@st.composite
def graphs(draw: st.DrawFn) -> Graph:
    """Return a finite prerequisite graph over the known goal names.

    Cycles are neither excluded nor forced: prerequisites are drawn from every
    name, so a goal may name itself or two goals may name each other. Both are
    legal in a Makefile, and both are shapes the recursive walk has to survive.

    Returns
    -------
    Graph
        Goal to its ordinary and order-only prerequisites.
    """
    goals = draw(
        st.lists(st.sampled_from(GOAL_NAMES), min_size=1, max_size=4, unique=True)
    )
    declares_check = draw(st.booleans())
    if declares_check:
        goals.append(CAPABILITY_TARGET)

    graph: Graph = {}
    for goal in goals:
        names = st.sampled_from(NODE_NAMES)
        ordinary = draw(st.lists(names, max_size=3, unique=True))
        order_only = draw(st.lists(names, max_size=2, unique=True))
        graph[goal] = (tuple(ordinary), tuple(order_only))
    return graph


#: A goal reaching the check through a chain, alongside one that reaches
#: nothing: the disconnected case, which a reading that gated everything would
#: fail.
CHAIN_AND_ISLAND: typ.Final[Graph] = {
    "lint": (("lint-clippy",), ()),
    "lint-clippy": ((CAPABILITY_TARGET,), ()),
    "bench-build": ((), ()),
}

#: The repository's own shape: a file rule carrying the check as an order-only
#: prerequisite, with a phony goal in front of it.
ORDER_ONLY_EDGE: typ.Final[Graph] = {
    "build": (("target/debug/$(APP)",), ()),
    "target/debug/$(APP)": ((), (CAPABILITY_TARGET,)),
}

#: Two goals reaching the check through one shared prerequisite.
SHARED_PATH: typ.Final[Graph] = {
    "build": (("lint-clippy",), ()),
    "test": (("lint-clippy",), ()),
    "lint-clippy": ((CAPABILITY_TARGET,), ()),
}

#: A cycle that reaches the check, and one that does not. The walk must answer
#: for both rather than recursing forever on either.
CYCLES: typ.Final[Graph] = {
    "build": (("test",), ()),
    "test": (("build", CAPABILITY_TARGET), ()),
    "lint": (("bench-build",), ()),
    "bench-build": (("lint",), ()),
}


@example(graph=CHAIN_AND_ISLAND)
@example(graph=ORDER_ONLY_EDGE)
@example(graph=SHARED_PATH)
@example(graph=CYCLES)
@given(graph=graphs())
def test_gated_targets_agrees_with_an_independent_closure(graph: Graph) -> None:
    """The reading reports exactly the goals that reach the capability check.

    Stated against a model computed the other way round, so agreement is
    evidence rather than a restatement. A walk that stopped at direct
    prerequisites would disagree on the chain, one that dropped order-only
    edges would disagree on the file rule, and one that abandoned a goal on
    meeting a cycle would disagree on the cyclic pair that does reach the
    check.
    """
    makefile = render(graph)
    reported = gated_targets(makefile)
    expected = reaches_capability(graph)
    assert reported == expected, (
        f"the reading and the closure disagree: {reported} against {expected}"
    )


@given(graph=graphs())
def test_every_declared_edge_is_read_back(graph: Graph) -> None:
    """Parsing returns each goal's declared prerequisites, order-only included.

    The reachability claim rests on this one: a walk over edges the reading
    never recorded would be correct about a graph that is not the file's.
    """
    parsed = rule_prerequisites(render(graph))
    assert set(parsed) == set(graph), (
        f"every declared goal should be read back, got {set(parsed)}"
    )
    for goal, (ordinary, order_only) in graph.items():
        declared = sorted([*ordinary, *order_only])
        assert sorted(parsed[goal]) == declared, (
            f"`{goal}` should carry {declared}, got {sorted(parsed[goal])}"
        )


@given(graph=graphs())
def test_a_goal_reaching_nothing_is_never_gated(graph: Graph) -> None:
    """A goal with no prerequisites is gated only when it is the check itself.

    The half that a reading gating everything would fail. Paired with the
    agreement property above, which a reading gating nothing would fail, so
    neither degenerate answer survives both.
    """
    assume(
        any(not ordinary and not order_only for ordinary, order_only in graph.values())
    )
    gated = gated_targets(render(graph))
    for goal, (ordinary, order_only) in graph.items():
        if ordinary or order_only:
            continue
        assert (goal in gated) == (goal == CAPABILITY_TARGET), (
            f"`{goal}` declares nothing, so only the check itself is gated"
        )
