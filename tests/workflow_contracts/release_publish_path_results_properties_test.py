"""Hold the release graph's derived ordering and results to their rules.

The scenario table exercises the graphs GitHub accepts. The complementary claim
is over any small graph: ``dependency_order`` places each settled job after the
jobs it needs, reports an unorderable graph exactly once, and still names every
stranded job; ``resolve_results`` then derives one result per job along that
order, honouring a declared leaf and propagating a non-successful prerequisite
to a job that declares no guard.

The expected answer is computed here from the generated edges by reachability,
independently of the ordering under test, so an order that drops a dependency
or a cycle report that names only a prefix of the unresolved jobs is a
generated counterexample rather than a hand-picked case someone forgot to add.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

from hypothesis import given
from hypothesis import strategies as st
from release_publish_path_results import dependency_order, resolve_results
from release_publish_path_scenario_model import Scenario

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: A name no generated job declares, standing for a reusable workflow's job.
GHOST = "ghost"


def _in_graph_needs(job: object, jobs: cabc.Mapping[str, object]) -> set[str]:
    """Return the job's needs that name another job in the same graph."""
    if not isinstance(job, dict):
        return set()
    declared = job.get("needs", [])
    names = [declared] if isinstance(declared, str) else declared
    if not isinstance(names, list):
        return set()
    return {name for name in names if isinstance(name, str)} & set(jobs)


def _reaches_a_cycle(name: str, needs: cabc.Mapping[str, set[str]]) -> bool:
    """Return whether the job can reach a cycle through its needs."""
    stack: list[tuple[str, frozenset[str]]] = [(name, frozenset())]
    while stack:
        current, path = stack.pop()
        if current in path:
            return True
        stack.extend((need, path | {current}) for need in needs[current])
    return False


def _needs_value(chosen: list[str], *, as_string: bool) -> object:
    """Return a generated ``needs`` in one of the spellings GitHub accepts."""
    return chosen[0] if as_string and len(chosen) == 1 else chosen


def _any_job_candidates(names: list[str], _index: int) -> list[str]:
    """Return every job name plus the ghost, so a graph may hold a cycle."""
    return [*names, GHOST]


def _earlier_job_candidates(names: list[str], index: int) -> list[str]:
    """Return the earlier job names plus the ghost, so a graph stays acyclic."""
    return [*names[:index], GHOST]


def _draw_graph(
    draw: st.DrawFn,
    candidates_for: cabc.Callable[[list[str], int], list[str]],
) -> dict[str, object]:
    """Draw a jobs graph, taking each job's candidate needs from a builder."""
    count = draw(st.integers(min_value=1, max_value=6))
    names = [f"job{index}" for index in range(count)]
    graphs: dict[str, object] = {}
    for index, name in enumerate(names):
        chosen = draw(
            st.lists(
                st.sampled_from(candidates_for(names, index)),
                unique=True,
                max_size=3,
            )
        )
        graphs[name] = {"needs": _needs_value(chosen, as_string=draw(st.booleans()))}
    # Declaring the jobs out of dependency order keeps a resolver that walks the
    # mapping directly from passing just because declaration order happens to
    # be a valid order.
    return {name: graphs[name] for name in draw(st.permutations([*graphs]))}


@st.composite
def _graphs(draw: st.DrawFn) -> dict[str, object]:
    """Draw a graph whose needs may be a string, a list, or form a cycle."""
    return _draw_graph(draw, _any_job_candidates)


@st.composite
def _acyclic_graphs(draw: st.DrawFn) -> dict[str, object]:
    """Draw a graph whose jobs may only need jobs declared before them."""
    return _draw_graph(draw, _earlier_job_candidates)


@given(graphs=_graphs())
def test_dependency_order_places_every_settled_job_after_its_needs(
    graphs: dict[str, object],
) -> None:
    """Order settled jobs after their needs and report the stranded ones once."""
    needs = {name: _in_graph_needs(job, graphs) for name, job in graphs.items()}
    stranded = {name for name in graphs if _reaches_a_cycle(name, needs)}
    settled = set(graphs) - stranded
    violations: list[str] = []

    order = dependency_order(graphs, violations)

    assert sorted(order) == sorted(graphs), (
        f"every job must be named exactly once, got {order!r} for {sorted(graphs)!r}"
    )
    position = {name: index for index, name in enumerate(order)}
    for name in sorted(settled):
        for need in needs[name]:
            assert position[need] < position[name], (
                f"{need} must precede {name} in {order!r}"
            )
    assert set(order[: len(settled)]) == settled, (
        f"the settled jobs must be ordered together, got {order!r}"
    )
    if stranded:
        assert violations == [
            f"release.jobs: dependency cycle among {sorted(stranded)!r}"
        ], f"the stranded jobs must be reported once, got {violations!r}"
        assert order[len(settled) :] == sorted(stranded), (
            f"the stranded jobs must follow in sorted order, got {order!r}"
        )
    else:
        assert not violations, (
            f"an acyclic graph must not be reported, got {violations!r}"
        )


@given(graphs=_acyclic_graphs())
def test_resolve_results_derives_one_result_per_job(
    graphs: dict[str, object],
) -> None:
    """Derive each job's result along the order and propagate a leaf failure."""
    needs = {name: _in_graph_needs(job, graphs) for name, job in graphs.items()}
    declared = {"job0": "failure"}
    scenario = Scenario(
        name="generated",
        mode="publish",
        event_action="",
        needs_results=declared,
        cancelled=False,
        release_runs=True,
    )
    violations: list[str] = []

    results = resolve_results({"jobs": graphs}, scenario, {}, violations)

    assert not violations, f"an acyclic graph must not be reported, got {violations!r}"
    assert sorted(results) == sorted(graphs), (
        f"every job must have a result, got {sorted(results)!r}"
    )
    for name in dependency_order(graphs, violations):
        expected = (
            declared.get(name, "success")
            if all(results[need] == "success" for need in needs[name])
            else "skipped"
        )
        assert results[name] == expected, (
            f"{name} must resolve to {expected!r}, got {results[name]!r} in {results!r}"
        )
