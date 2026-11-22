//! Coverage for the generated output-directory preparation statements.
//!
//! The cases pin which edges get preparation, what the rule and edge look like,
//! and the ordering that makes a prerequisite's own directory creation safe.
//! The real-Ninja scenarios in `tests/phony_always_runtime_tests.rs` prove the
//! same statements execute as intended.

use super::super::test_support::command_action;
use super::{mkdir_script, nodes, rule_name};
use crate::ast::StringOrList;
use crate::ir::{BuildEdge, BuildGraph, DependencyOrder, EdgeOutputVisibility};
use crate::ninja_gen::{RecipeShell, generate_with_shell};
use camino::{Utf8Path, Utf8PathBuf};
use rstest::rstest;

/// The parts of a fixture edge that vary between the preparation cases.
#[derive(Clone, Copy, Default)]
struct EdgeShape<'a> {
    /// Explicit outputs, in declaration order.
    outputs: &'a [&'a Utf8Path],
    /// Ordinary inputs.
    inputs: &'a [&'a Utf8Path],
    /// Implicit dependencies.
    implicit_deps: &'a [&'a Utf8Path],
    /// Order-only dependencies.
    order_only_deps: &'a [&'a Utf8Path],
    /// Whether the dependencies run in declaration order.
    serial: bool,
    /// Whether the edge declares itself `phony`.
    phony: bool,
}

/// Insert one edge with the supplied shape.
fn insert_edge(graph: &mut BuildGraph, action_id: &str, shape: EdgeShape<'_>) {
    graph
        .insert_edge(BuildEdge {
            action_id: action_id.to_owned(),
            inputs: shape.inputs.iter().map(|path| path.to_path_buf()).collect(),
            implicit_deps: shape
                .implicit_deps
                .iter()
                .map(|path| path.to_path_buf())
                .collect(),
            dependency_order: if shape.serial {
                DependencyOrder::Serial
            } else {
                DependencyOrder::Parallel
            },
            explicit_outputs: shape
                .outputs
                .iter()
                .map(|path| path.to_path_buf())
                .collect(),
            implicit_outputs: Vec::new(),
            order_only_deps: shape
                .order_only_deps
                .iter()
                .map(|path| path.to_path_buf())
                .collect(),
            progress_label: None,
            phony: shape.phony,
            always: false,
            output_visibility: EdgeOutputVisibility::default(),
        })
        .expect("fixture outputs are unique");
}

/// Generate one graph whose single edge has the supplied shape.
fn ninja_for(outputs: &[&Utf8Path], inputs: &[&Utf8Path], phony: bool) -> String {
    let mut graph = BuildGraph::default();
    graph.actions.insert(
        "run".to_owned(),
        command_action(StringOrList::String("printf 'x' > report".to_owned())),
    );
    insert_edge(
        &mut graph,
        "run",
        EdgeShape {
            outputs,
            inputs,
            phony,
            ..EdgeShape::default()
        },
    );
    generate_with_shell(&graph, RecipeShell::Posix).expect("the fixture graph should generate")
}

/// Return the first generated line starting with `prefix`.
fn line_starting_with<'a>(ninja: &'a str, prefix: &str) -> &'a str {
    let Some(line) = ninja.lines().find(|line| line.starts_with(prefix)) else {
        panic!("no generated line starts with {prefix:?}:\n{ninja}");
    };
    line
}

/// Return the prerequisites of one build line, in order.
fn prerequisites(edge: &str) -> Vec<&str> {
    // `map_or_default` is still unstable on the lint toolchain, so the
    // absent-prerequisite case spells its empty vector out.
    edge.split_once(": ")
        .and_then(|(_, rest)| rest.split_once(' '))
        .map_or(Vec::new(), |(_, rest)| rest.split_whitespace().collect())
}

/// Count the generated lines starting with `prefix`.
fn count_lines(ninja: &str, prefix: &str) -> usize {
    ninja
        .lines()
        .filter(|line| line.starts_with(prefix))
        .count()
}

#[rstest]
#[case::nested_phony(&[Utf8Path::new("build/report")], true, true)]
#[case::bare_phony(&[Utf8Path::new("report")], true, false)]
#[case::nested_ordinary(&[Utf8Path::new("build/report")], false, false)]
fn only_a_nested_phony_output_is_prepared(
    #[case] outputs: &[&Utf8Path],
    #[case] phony: bool,
    #[case] prepared: bool,
) {
    let ninja = ninja_for(outputs, &[], phony);
    assert_eq!(
        ninja.contains(".netsuke/phony/dir-"),
        prepared,
        "preparation for phony={phony} {outputs:?} must be {prepared}:\n{ninja}",
    );
}

/// A dependency-only action runs no recipe, so its names need no directory.
#[test]
fn a_dependency_only_edge_is_never_prepared() {
    let mut graph = BuildGraph::default();
    graph
        .actions
        .insert("group".to_owned(), command_action(StringOrList::Empty));
    insert_edge(
        &mut graph,
        "group",
        EdgeShape {
            outputs: &[Utf8Path::new("build/report")],
            phony: true,
            ..EdgeShape::default()
        },
    );
    let ninja =
        generate_with_shell(&graph, RecipeShell::Posix).expect("the fixture should generate");
    assert!(
        !ninja.contains(".netsuke/phony/dir-"),
        "a dependency-only edge must not be prepared:\n{ninja}",
    );
}

#[test]
fn a_nested_phony_output_gets_a_rule_and_a_node() {
    let ninja = ninja_for(&[Utf8Path::new("build/report")], &[], true);
    let rule = line_starting_with(&ninja, "rule netsuke_mkdir_");
    let name = rule
        .strip_prefix("rule ")
        .expect("the preparation line starts with the rule keyword");
    assert!(
        ninja.contains("  command = mkdir -p -- build"),
        "the rule must create the directory with the owning shell:\n{ninja}",
    );
    let edge = line_starting_with(&ninja, "build .netsuke/phony/dir-");
    assert!(
        edge.ends_with(&format!(": {name}")),
        "the node must be built by the directory's own rule: {edge}",
    );
    let hidden = line_starting_with(&ninja, "build .netsuke/phony/edge-");
    assert!(
        hidden.contains(" || .netsuke/phony/dir-"),
        "the recipe statement must depend on the prepared directory: {hidden}",
    );
}

/// Two recipes writing into one directory can share its rule but not its node.
#[test]
fn edges_sharing_a_directory_share_one_rule_and_keep_distinct_nodes() {
    let mut graph = BuildGraph::default();
    graph
        .actions
        .insert("first".to_owned(), command_action("true".into()));
    graph
        .actions
        .insert("second".to_owned(), command_action("true".into()));
    insert_edge(
        &mut graph,
        "first",
        EdgeShape {
            outputs: &[Utf8Path::new("build/one")],
            phony: true,
            ..EdgeShape::default()
        },
    );
    insert_edge(
        &mut graph,
        "second",
        EdgeShape {
            outputs: &[Utf8Path::new("build/two")],
            phony: true,
            ..EdgeShape::default()
        },
    );
    let ninja =
        generate_with_shell(&graph, RecipeShell::Posix).expect("the fixture should generate");
    assert_eq!(
        count_lines(&ninja, "rule netsuke_mkdir_"),
        1,
        "one directory must be created by one rule:\n{ninja}",
    );
    assert_eq!(
        count_lines(&ninja, "build .netsuke/phony/dir-"),
        2,
        "each edge must own its own node:\n{ninja}",
    );
}

/// One recipe naming two outputs in one directory needs one node for it.
#[test]
fn one_edge_naming_one_directory_twice_gets_one_node() {
    let mut graph = BuildGraph::default();
    graph
        .actions
        .insert("run".to_owned(), command_action("true".into()));
    graph
        .insert_edge(BuildEdge {
            action_id: "run".to_owned(),
            inputs: Vec::new(),
            implicit_deps: Vec::new(),
            dependency_order: DependencyOrder::Parallel,
            explicit_outputs: vec![Utf8PathBuf::from("build/one")],
            implicit_outputs: vec![Utf8PathBuf::from("build/two")],
            order_only_deps: Vec::new(),
            progress_label: None,
            phony: true,
            always: false,
            output_visibility: EdgeOutputVisibility::default(),
        })
        .expect("fixture outputs are unique");
    let ninja =
        generate_with_shell(&graph, RecipeShell::Posix).expect("the fixture should generate");
    assert_eq!(
        count_lines(&ninja, "build .netsuke/phony/dir-"),
        1,
        "one directory needs one node:\n{ninja}",
    );
}

/// Preparation runs after every prerequisite role the recipe declares.
///
/// A prerequisite may create the same directory itself, so preparation must not
/// be scheduled ahead of one. Order-only prerequisites of the recipe are normal
/// prerequisites here for the same reason.
#[test]
fn preparation_depends_on_the_recipe_prerequisites_in_every_role() {
    let mut graph = BuildGraph::default();
    graph
        .actions
        .insert("run".to_owned(), command_action("true".into()));
    insert_edge(
        &mut graph,
        "run",
        EdgeShape {
            outputs: &[Utf8Path::new("build/report")],
            inputs: &[Utf8Path::new("src/input.txt")],
            implicit_deps: &[Utf8Path::new("gen/generated.h")],
            order_only_deps: &[Utf8Path::new("stamp")],
            phony: true,
            ..EdgeShape::default()
        },
    );
    let ninja =
        generate_with_shell(&graph, RecipeShell::Posix).expect("the fixture should generate");
    let edge = line_starting_with(&ninja, "build .netsuke/phony/dir-");
    assert_eq!(
        prerequisites(edge),
        ["src/input.txt", "gen/generated.h", "stamp"],
        "preparation must wait for every prerequisite role: {edge}",
    );
}

/// A recipe with several inputs keeps them in declaration order.
#[test]
fn preparation_keeps_the_declaration_order_of_its_inputs() {
    let ninja = ninja_for(
        &[Utf8Path::new("build/report")],
        &[Utf8Path::new("b.txt"), Utf8Path::new("a.txt")],
        true,
    );
    let edge = line_starting_with(&ninja, "build .netsuke/phony/dir-");
    assert_eq!(
        prerequisites(edge),
        ["b.txt", "a.txt"],
        "prerequisites keep the edge's own order: {edge}",
    );
}

#[test]
fn a_directory_with_a_space_is_quoted_for_the_shell() {
    let directory = Utf8Path::new("out dir/reports");
    let script = mkdir_script(RecipeShell::Posix, directory);
    assert_eq!(
        script.as_str(),
        "mkdir -p -- out' dir/reports'",
        "a directory with a space must reach the shell as one word",
    );
    let power_shell = mkdir_script(RecipeShell::PowerShell, Utf8Path::new("out dir"));
    assert_eq!(
        power_shell.as_str(),
        "New-Item -ItemType Directory -Force -Path 'out dir' | Out-Null",
        "the PowerShell form must quote its path as a literal",
    );
}

/// The rule name is a pure function of the directory.
#[test]
fn the_rule_name_is_derived_from_the_directory_alone() {
    let first = rule_name(Utf8Path::new("build"));
    assert_eq!(
        first,
        rule_name(Utf8Path::new("build")),
        "the same directory must always name the same rule",
    );
    assert_ne!(
        first,
        rule_name(Utf8Path::new("out")),
        "different directories must not share a rule",
    );
    assert!(
        first.starts_with("netsuke_mkdir_")
            && first
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '_'),
        "the rule name must be a plain Ninja identifier: {first}",
    );
}

/// Preparation nodes are content-addressed from the edge and the directory.
#[test]
fn preparation_nodes_are_derived_from_the_edge() {
    let mut graph = BuildGraph::default();
    graph
        .actions
        .insert("run".to_owned(), command_action("true".into()));
    insert_edge(
        &mut graph,
        "run",
        EdgeShape {
            outputs: &[Utf8Path::new("build/report")],
            phony: true,
            ..EdgeShape::default()
        },
    );
    let edge = graph.edges().next().expect("the fixture declares one edge");
    let prepared = nodes(&graph, edge);
    assert_eq!(prepared.len(), 1, "one directory yields one node");
    let node = prepared.first().expect("one directory yields one node");
    assert!(
        node.as_str().starts_with(".netsuke/phony/dir-"),
        "a node must live in the reserved namespace: {node}",
    );
    assert_eq!(
        Some(node),
        nodes(&graph, edge).first(),
        "the identity must be stable for one graph",
    );
}

mod serial;

mod response_file;
