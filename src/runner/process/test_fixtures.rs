//! Shared graph fixtures for command-list process tests.

use crate::{
    ast::{CommandIsolation, Recipe, StringOrList},
    ir::{Action, BuildEdge, BuildGraph, DependencyOrder},
};

/// Stable action id used by command-list process tests.
pub(super) const TEST_ACTION_ID: &str = "identity";

/// Build a graph containing the shared command-list action.
pub(super) fn graph_with_identity_action() -> BuildGraph {
    let mut graph = BuildGraph::default();
    graph.actions.insert(
        TEST_ACTION_ID.to_owned(),
        Action {
            recipe: Recipe::Command {
                command: StringOrList::List(vec!["true".to_owned()]),
                isolation: CommandIsolation::Shared,
            },
            description: None,
            depfile: None,
            deps_format: None,
            pool: None,
            restat: false,
        },
    );
    graph
}

/// Insert one canonical edge with explicit outputs into `graph`.
pub(super) fn insert_edge(
    graph: &mut BuildGraph,
    action_id: &str,
    explicit_outputs: &[&str],
    phony: bool,
) {
    let result = graph.insert_edge(BuildEdge {
        action_id: action_id.to_owned(),
        inputs: Vec::new(),
        implicit_deps: Vec::new(),
        dependency_order: DependencyOrder::Parallel,
        explicit_outputs: explicit_outputs
            .iter()
            .map(|output| (*output).into())
            .collect(),
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        progress_label: None,
        phony,
        always: false,
        console: false,
    });
    assert!(result.is_ok(), "test edge outputs should be unique");
}

/// Add a single explicit output for the shared command-list action.
pub(super) fn add_target_edge(graph: &mut BuildGraph, target: &str) {
    insert_edge(graph, TEST_ACTION_ID, &[target], false);
}
