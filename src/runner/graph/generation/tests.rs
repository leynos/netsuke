//! Unit coverage for the directories a CLI build prepares before Ninja runs.
//!
//! The collection keeps a `phony` recipe's nested output working after the
//! lowering moved its product to a hidden path: Ninja no longer sees the
//! declared output, so it no longer creates that output's directory. These
//! cases pin which edges contribute and how the set is deduplicated; the
//! real-Ninja runtime tests prove the resulting build succeeds.

use super::phony_output_dirs;
use crate::ast::{CommandIsolation, Recipe, StringOrList};
use crate::ir::{Action, BuildEdge, BuildGraph, DependencyOrder, EdgeOutputVisibility};
use camino::Utf8PathBuf;
use rstest::rstest;

/// Build an action whose recipe is either a command or nothing at all.
fn action(recipe: Recipe) -> Action {
    Action {
        recipe,
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    }
}

/// Build a command recipe that Ninja would execute.
fn command() -> Recipe {
    Recipe::Command {
        command: StringOrList::String("true".to_owned()),
        isolation: CommandIsolation::Shared,
    }
}

/// Insert an edge referencing `action_id` and naming `outputs` explicitly.
fn insert_edge(graph: &mut BuildGraph, action_id: &str, outputs: &[&str], phony: bool) {
    let explicit_outputs = outputs.iter().map(Utf8PathBuf::from).collect();
    graph
        .insert_edge(BuildEdge {
            action_id: action_id.to_owned(),
            inputs: Vec::new(),
            implicit_deps: Vec::new(),
            dependency_order: DependencyOrder::Parallel,
            explicit_outputs,
            implicit_outputs: Vec::new(),
            order_only_deps: Vec::new(),
            progress_label: None,
            phony,
            always: false,
            output_visibility: EdgeOutputVisibility::default(),
        })
        .expect("fixture outputs are unique");
}

/// Insert an edge whose action carries the supplied definition.
///
/// The action is registered under the edge's first output, which the fixture
/// graphs declare uniquely. The graph only requires the two ids to match, so
/// deriving one keeps a redundant name out of every case.
fn insert_action_edge(graph: &mut BuildGraph, definition: Action, outputs: &[&str], phony: bool) {
    let action_id = outputs
        .first()
        .copied()
        .expect("fixture edges declare an output");
    graph.actions.insert(action_id.to_owned(), definition);
    insert_edge(graph, action_id, outputs, phony);
}

#[rstest]
#[case::nested_phony_output(&["build/report"], true, &["build"])]
#[case::bare_phony_output(&["report"], true, &[])]
#[case::nested_ordinary_output(&["build/report"], false, &[])]
fn an_executable_phony_edge_contributes_its_output_parent(
    #[case] outputs: &[&str],
    #[case] phony: bool,
    #[case] expected: &[&str],
) {
    let mut graph = BuildGraph::default();
    insert_action_edge(&mut graph, action(command()), outputs, phony);
    assert_eq!(
        phony_output_dirs(&graph),
        expected.iter().map(Utf8PathBuf::from).collect::<Vec<_>>(),
        "an executable phony edge must contribute exactly its output parents",
    );
}

/// A dependency-only action renders Ninja's own `phony` rule, so Ninja already
/// treats its names as aggregates and creates nothing for them.
#[test]
fn a_dependency_only_edge_contributes_nothing() {
    let mut graph = BuildGraph::default();
    insert_action_edge(
        &mut graph,
        action(Recipe::Command {
            command: StringOrList::Empty,
            isolation: CommandIsolation::Shared,
        }),
        &["build/report"],
        true,
    );
    assert_eq!(
        phony_output_dirs(&graph),
        Vec::<Utf8PathBuf>::new(),
        "a dependency-only edge must not claim a directory",
    );
}

/// Generation reports a missing action, so the collection skips that edge
/// rather than inventing a lowering for it.
#[test]
fn an_edge_without_an_action_contributes_nothing() {
    let mut graph = BuildGraph::default();
    insert_edge(&mut graph, "missing", &["build/report"], true);
    assert_eq!(
        phony_output_dirs(&graph),
        Vec::<Utf8PathBuf>::new(),
        "an edge without an action must not claim a directory",
    );
}

/// An implicit output is published by the same alias, so its parent needs the
/// same preparation.
#[test]
fn an_implicit_output_contributes_its_parent() {
    let mut graph = BuildGraph::default();
    graph.actions.insert("run".to_owned(), action(command()));
    graph
        .insert_edge(BuildEdge {
            action_id: "run".to_owned(),
            inputs: Vec::new(),
            implicit_deps: Vec::new(),
            dependency_order: DependencyOrder::Parallel,
            explicit_outputs: vec![Utf8PathBuf::from("report")],
            implicit_outputs: vec![Utf8PathBuf::from("gen/header.h")],
            order_only_deps: Vec::new(),
            progress_label: None,
            phony: true,
            always: false,
            output_visibility: EdgeOutputVisibility::default(),
        })
        .expect("fixture outputs are unique");
    assert_eq!(
        phony_output_dirs(&graph),
        vec![Utf8PathBuf::from("gen")],
        "an implicit output must contribute its parent",
    );
}

/// Two edges sharing a directory must not create it twice, and the order must
/// not depend on the order the edges were inserted.
#[test]
fn shared_directories_are_created_once_in_a_stable_order() {
    let mut graph = BuildGraph::default();
    insert_action_edge(&mut graph, action(command()), &["out/second"], true);
    insert_action_edge(&mut graph, action(command()), &["build/first"], true);
    insert_action_edge(&mut graph, action(command()), &["build/third"], true);
    assert_eq!(
        phony_output_dirs(&graph),
        vec![Utf8PathBuf::from("build"), Utf8PathBuf::from("out")],
        "shared directories must be deduplicated and sorted",
    );
}
