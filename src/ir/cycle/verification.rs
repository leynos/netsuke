//! Kani harnesses for bounded IR cycle-handling properties.

use camino::Utf8PathBuf;

use super::super::graph::{BuildEdge, BuildGraph};
use super::contains_cycle;

mod support;

/// Prove a self-dependency reports a cycle and no missing dependency.
#[kani::proof]
#[kani::solver(kissat)]
#[kani::unwind(5)]
fn self_dependency_reports_cycle() {
    let mut graph = BuildGraph::default();
    graph.insert_edge(edge("a", deps("a"), Vec::new()));
    kani::assume(graph.output_count() == 1 && graph.edge_count() == 1);

    kani::assert(contains_cycle(&graph), "self-dependency reports a cycle");
}

/// Prove a two-node cycle is detected when `a` is inserted first.
#[kani::proof]
#[kani::solver(kissat)]
#[kani::unwind(5)]
fn two_node_cycle_reports_cycle_a_first() {
    let mut graph = BuildGraph::default();
    graph.insert_edge(edge("a", deps("b"), Vec::new()));
    graph.insert_edge(edge("b", deps("a"), Vec::new()));
    kani::assume(graph.output_count() == 2 && graph.edge_count() == 2);

    kani::assert(contains_cycle(&graph), "two-node cycle is rejected");
}

/// Prove a two-node cycle is detected when `b` is inserted first.
#[kani::proof]
#[kani::solver(kissat)]
#[kani::unwind(5)]
fn two_node_cycle_reports_cycle_b_first() {
    let mut graph = BuildGraph::default();
    graph.insert_edge(edge("b", deps("a"), Vec::new()));
    graph.insert_edge(edge("a", deps("b"), Vec::new()));
    kani::assume(graph.output_count() == 2 && graph.edge_count() == 2);

    kani::assert(contains_cycle(&graph), "two-node cycle is rejected");
}

/// Prove that a dependency naming an implicit output reaches its producer.
#[kani::proof]
#[kani::solver(kissat)]
#[kani::unwind(6)]
fn implicit_output_alias_cycle_reports_cycle() {
    let graph = two_edge_graph(
        edge_with(EdgeSpec {
            output: "a",
            inputs: deps("b"),
            implicit_deps: Vec::new(),
            implicit_outputs: deps("c"),
            order_only_deps: Vec::new(),
        }),
        edge("b", deps("c"), Vec::new()),
    );
    // Keep the state space independent of the alias index being tested.
    kani::assume(graph.edge_count() == 2);
    kani::assert(
        contains_cycle(&graph),
        "cycle through an implicit output is detected",
    );
}

/// Prove a two-node cycle closed by an order-only dependency is detected.
#[kani::proof]
#[kani::solver(kissat)]
#[kani::unwind(6)]
fn order_only_two_node_cycle_reports_cycle() {
    let graph = two_edge_graph(
        edge_with(EdgeSpec {
            output: "a",
            inputs: Vec::new(),
            implicit_deps: Vec::new(),
            implicit_outputs: Vec::new(),
            order_only_deps: deps("b"),
        }),
        edge("b", deps("a"), Vec::new()),
    );
    kani::assume(graph.output_count() == 2 && graph.edge_count() == 2);
    kani::assert(
        contains_cycle(&graph),
        "cycle closed by an order-only dependency is detected",
    );
}

/// Assert that the given target graph contains no cycle.
fn assert_no_cycle(graph: &BuildGraph, _msg: &'static str) {
    kani::assert(!contains_cycle(graph), "missing dependency is not a cycle");
}

/// Prove an absent direct dependency is not cyclic.
#[kani::proof]
#[kani::solver(kissat)]
#[kani::unwind(6)]
fn direct_missing_dependency_does_not_report_cycle() {
    let mut graph = BuildGraph::default();
    graph.insert_edge(edge("a", deps("c"), Vec::new()));
    kani::assume(graph.output_count() == 1 && graph.edge_count() == 1);

    assert_no_cycle(&graph, "direct missing dependency is not a cycle");
}

/// Prove an absent dependency beyond a present target is not cyclic.
#[kani::proof]
#[kani::solver(kissat)]
#[kani::unwind(6)]
fn transitive_missing_dependency_does_not_report_cycle() {
    let mut graph = BuildGraph::default();
    graph.insert_edge(edge("a", deps("b"), Vec::new()));
    graph.insert_edge(edge("b", deps("c"), Vec::new()));
    kani::assume(graph.output_count() == 2 && graph.edge_count() == 2);

    assert_no_cycle(&graph, "transitive missing dependency is not a cycle");
}

/// Prove two-node canonicalization preserves the canonical cycle contract.
#[kani::proof]
#[kani::solver(kissat)]
#[kani::unwind(6)]
fn canonicalize_two_node_cycle_is_canonical() {
    support::assert_kernel_canonical_properties(support::closed_two_node_id_cycle());
}

/// Prove three-node canonicalization preserves the canonical cycle contract.
#[kani::proof]
#[kani::solver(kissat)]
#[kani::unwind(6)]
fn canonicalize_three_node_cycle_is_canonical() {
    support::assert_kernel_canonical_properties(support::closed_three_node_id_cycle());
}

/// Prove four-node canonicalization preserves the canonical cycle contract.
#[kani::proof]
#[kani::solver(kissat)]
#[kani::unwind(6)]
fn canonicalize_four_node_cycle_is_canonical() {
    support::assert_kernel_canonical_properties(support::closed_four_node_id_cycle());
}

/// Prove the path wrapper agrees with the generic kernel for two-node cycles.
#[kani::proof]
#[kani::solver(kissat)]
#[kani::unwind(6)]
fn canonicalize_path_wrapper_matches_u8_kernel_for_two_nodes() {
    support::assert_path_wrapper_matches_kernel();
}

/// Build a Kani graph edge without additional output aliases or order-only deps.
fn edge(output: &str, inputs: Vec<Utf8PathBuf>, implicit_deps: Vec<Utf8PathBuf>) -> BuildEdge {
    edge_with(EdgeSpec {
        output,
        inputs,
        implicit_deps,
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
    })
}

/// Assemble two test edges without changing the graph's output aliases.
fn two_edge_graph(first: BuildEdge, second: BuildEdge) -> BuildGraph {
    let mut graph = BuildGraph::default();
    graph.insert_edge(first);
    graph.insert_edge(second);
    graph
}

/// Hold path fields for one private Kani cycle-fixture edge.
struct EdgeSpec<'a> {
    /// Name the edge's primary explicit output.
    output: &'a str,
    /// Name explicit input dependencies.
    inputs: Vec<Utf8PathBuf>,
    /// Name implicit input dependencies.
    implicit_deps: Vec<Utf8PathBuf>,
    /// Name implicit output aliases.
    implicit_outputs: Vec<Utf8PathBuf>,
    /// Name order-only input dependencies.
    order_only_deps: Vec<Utf8PathBuf>,
}

/// Build a Kani graph edge from its test-only fixture specification.
fn edge_with(spec: EdgeSpec<'_>) -> BuildEdge {
    BuildEdge {
        action_id: "id".to_owned(),
        inputs: spec.inputs,
        implicit_deps: spec.implicit_deps,
        dependency_order: crate::ir::DependencyOrder::Parallel,
        explicit_outputs: vec![path(spec.output)],
        implicit_outputs: spec.implicit_outputs,
        order_only_deps: spec.order_only_deps,
        phony: false,
        always: false,
    }
}

/// Convert a dependency name into a one-element UTF-8 path list.
fn deps(dependency: &str) -> Vec<Utf8PathBuf> {
    vec![path(dependency)]
}

/// Convert a name into a UTF-8 path.
///
/// Keep names to one byte because the Kani-bounded graph map treats each path
/// as an identifier rather than traversing Camino's full path parser.
fn path(name: &str) -> Utf8PathBuf {
    Utf8PathBuf::from(name)
}
