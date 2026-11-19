//! Generation-side coverage for the `phony` and `always` lowerings.
//!
//! These tests read the emitted Ninja text; the runtime behaviour the text is
//! meant to produce is proved separately by `tests/phony_always_runtime_tests.rs`
//! against a real Ninja. Splitting the two keeps a formatting regression
//! reporting a text diff rather than a mysteriously failing build.
//!
//! The lowering has three parts: an executable `phony` edge carries its recipe
//! on a hidden output beneath `.netsuke/phony` and republishes its declared
//! names as a native `phony` alias, an `always` edge keeps its declared output
//! but gains a permanently dirty prerequisite, and both share one force
//! statement that is emitted only when some edge consumes it.

use camino::Utf8PathBuf;
use netsuke::ast::{CommandIsolation, Recipe, StringOrList};
use netsuke::ir::{Action, BuildEdge, BuildGraph, DependencyOrder, EdgeOutputVisibility};
use netsuke::ninja_gen::{
    NinjaGenError, RecipeShell, generate, generate_bundle, generate_with_shell,
};
use rstest::rstest;

/// The shared, permanently dirty prerequisite named by every forced edge.
const FORCE_PATH: &str = ".netsuke/phony/force";
/// The statement that declares the force prerequisite.
const FORCE_STATEMENT: &str = "build .netsuke/phony/force: phony";
/// Prefix of the hidden output that carries an executable `phony` edge.
const HIDDEN_PREFIX: &str = ".netsuke/phony/edge-";

/// Build an action that runs `command` under shared isolation.
///
/// Every action in this suite differs only in its recipe and `restat` flag, so
/// one constructor keeps the metadata defaults from being restated per case.
fn action_with(command: StringOrList, restat: bool) -> Action {
    Action {
        recipe: Recipe::Command {
            command,
            isolation: CommandIsolation::Shared,
        },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat,
    }
}

/// Build an executable shared-isolation command action.
fn command_action(command: &str, restat: bool) -> Action {
    action_with(command.into(), restat)
}

/// Build an action that lowers to Ninja's built-in `phony` rule.
fn dependency_only_action() -> Action {
    action_with(StringOrList::Empty, false)
}

/// Build a single-output edge over one explicit input.
fn edge(action_id: &str, output: &str, phony: bool, always: bool) -> BuildEdge {
    BuildEdge {
        action_id: action_id.into(),
        inputs: vec![Utf8PathBuf::from("in")],
        implicit_deps: Vec::new(),
        dependency_order: DependencyOrder::Parallel,
        explicit_outputs: vec![Utf8PathBuf::from(output)],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        progress_label: None,
        phony,
        always,
        output_visibility: EdgeOutputVisibility::default(),
    }
}

/// Register `action` under the edge's action id and insert the edge.
fn graph_of(action: Action, edge: BuildEdge) -> BuildGraph {
    let mut graph = BuildGraph::default();
    graph.actions.insert(edge.action_id.clone(), action);
    graph
        .insert_edge(edge)
        .expect("test graph output aliases must be unique");
    graph
}

/// Render a graph with explicit POSIX shell semantics.
fn generate_posix(graph: &BuildGraph) -> String {
    generate_with_shell(graph, RecipeShell::Posix).expect("generate POSIX Ninja manifest")
}

/// Collect the statement block beginning with `header`.
///
/// Ninja separates statements with a blank line, so a block is everything from
/// its header up to the next blank line. Reading a block rather than searching
/// the whole file keeps rule-level bindings out of edge-level assertions.
fn statement(ninja: &str, header: &str) -> String {
    ninja
        .lines()
        .skip_while(|line| !line.starts_with(header))
        .take_while(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Read the hidden output a `phony` edge's recipe statement targets.
fn hidden_output(ninja: &str) -> String {
    ninja
        .lines()
        .find_map(|line| line.strip_prefix("build .netsuke/phony/edge-"))
        .and_then(|rest| rest.split_once(':'))
        .map(|(digest, _)| format!("{HIDDEN_PREFIX}{digest}"))
        .expect("a phony executable edge must emit a hidden statement")
}

#[rstest]
fn phony_executable_edge_moves_its_recipe_onto_a_hidden_output() {
    let graph = graph_of(command_action("true", false), edge("a", "out", true, false));

    let ninja = generate_posix(&graph);
    let hidden = hidden_output(&ninja);

    assert!(
        ninja.starts_with("rule a\n  command = true\n\n"),
        "the executable action must still declare its own rule: {ninja}"
    );
    assert!(
        ninja.contains(&format!("{FORCE_STATEMENT}\n\n")),
        "the force statement must precede the edges that use it: {ninja}"
    );
    assert_eq!(
        ninja
            .matches(&format!("build {hidden}: a in | {FORCE_PATH}\n"))
            .count(),
        1,
        "the recipe must run against the hidden output, forced on every invocation: {ninja}"
    );
    assert_eq!(
        ninja
            .matches(&format!("build out: phony {hidden}\n"))
            .count(),
        1,
        "the declared name must be a native phony alias over the hidden output: {ninja}"
    );
    assert!(
        !ninja.contains("rule phony\n"),
        "the alias must use Ninja's built-in phony rule, not a generated one: {ninja}"
    );
}

#[rstest]
fn phony_alias_keeps_implicit_outputs_off_the_hidden_statement() {
    let edge = BuildEdge {
        implicit_outputs: vec![Utf8PathBuf::from("out.d")],
        ..edge("a", "out", true, false)
    };
    let graph = graph_of(command_action("true", false), edge);

    let ninja = generate_posix(&graph);
    let hidden = hidden_output(&ninja);

    assert_eq!(
        ninja
            .matches(&format!("build out | out.d: phony {hidden}\n"))
            .count(),
        1,
        "the alias must declare both explicit and implicit outputs: {ninja}"
    );
    assert_eq!(
        ninja
            .matches(&format!("build {hidden}: a in | {FORCE_PATH}\n"))
            .count(),
        1,
        "the hidden statement must not also claim the implicit output: {ninja}"
    );
}

#[rstest]
fn hidden_statement_keeps_bindings_but_suppresses_restat() {
    // The progress label is edge-level: `Action::description` names the rule
    // and is never rendered per edge, so setting it here would prove nothing.
    let edge = BuildEdge {
        output_visibility: EdgeOutputVisibility::Immediate,
        progress_label: Some("Building".to_owned()),
        ..edge("a", "out", true, true)
    };
    let graph = graph_of(command_action("true", false), edge);

    let ninja = generate_posix(&graph);
    let hidden = hidden_output(&ninja);
    let block = statement(&ninja, &format!("build {hidden}: "));

    assert!(
        block.contains("  description = "),
        "progress output must still name the public operation: {block}"
    );
    assert!(
        block.contains("  pool = console"),
        "an immediate edge must keep its console binding on the hidden statement: {block}"
    );
    assert!(
        !block.contains("restat = 1"),
        "the hidden output is never materialised, so restat cannot prune anything: {block}"
    );
}

#[rstest]
fn hidden_statement_clears_an_inherited_restat() {
    // The action's rule carries `restat = 1`, and an edge inherits its rule's
    // bindings unless it overrides them. The hidden statement must clear it:
    // its output is never created, so a surviving `restat` would let Ninja
    // treat the next invocation's dependants as up to date.
    let graph = graph_of(command_action("true", true), edge("a", "out", true, false));

    let ninja = generate_posix(&graph);
    let hidden = hidden_output(&ninja);
    let block = statement(&ninja, &format!("build {hidden}: "));

    assert!(
        block.lines().any(|line| line.trim_end() == "  restat ="),
        "the hidden statement must clear the rule's inherited restat: {block}"
    );
    assert!(
        !block.lines().any(|line| line.trim() == "restat = 1"),
        "the hidden statement must not re-state a restat override: {block}"
    );
}

#[rstest]
#[case::edge_supplied_restat(false, true)]
#[case::action_supplied_restat(true, false)]
fn always_edge_keeps_its_output_and_reports_restat(
    #[case] action_restat: bool,
    #[case] expects_edge_restat: bool,
) {
    let graph = graph_of(
        command_action("true", action_restat),
        edge("a", "out", false, true),
    );

    let ninja = generate_posix(&graph);
    let block = statement(&ninja, "build out: ");
    let expected_header = format!("build out: a in | {FORCE_PATH}");

    assert!(
        ninja.contains(&format!("{FORCE_STATEMENT}\n\n")),
        "the force statement is emitted because an edge consumes it: {ninja}"
    );
    assert_eq!(
        block.lines().next(),
        Some(expected_header.as_str()),
        "an always edge keeps its declared output and gains the force node: {block}"
    );
    assert_eq!(
        block.contains("  restat = 1"),
        expects_edge_restat,
        "the edge-level restat override follows the action's own restat flag: {block}"
    );
    assert!(
        !ninja.contains(HIDDEN_PREFIX),
        "an always but non-phony edge needs no hidden output: {ninja}"
    );
}

#[rstest]
fn plain_edge_emits_no_force_state() {
    let graph = graph_of(
        command_action("true", false),
        edge("a", "out", false, false),
    );

    let ninja = generate_posix(&graph);

    assert_eq!(
        ninja, "rule a\n  command = true\n\nbuild out: a in\n\n",
        "a graph that forces nothing must keep its previous byte output"
    );
}

#[rstest]
fn dependency_only_aggregate_ignores_the_phony_flag() {
    let edge = BuildEdge {
        inputs: Vec::new(),
        implicit_deps: vec![Utf8PathBuf::from("dep")],
        ..edge("aggregate", "all", true, false)
    };
    let graph = graph_of(dependency_only_action(), edge);

    let ninja = generate_posix(&graph);

    assert_eq!(
        ninja, "build all: phony | dep\n\n",
        "a dependency-only aggregate already is a phony node and must not gain a recipe edge"
    );
}

#[rstest]
fn hidden_outputs_do_not_depend_on_insertion_order() {
    let mut first = BuildGraph::default();
    for output in ["alpha", "beta"] {
        first
            .actions
            .insert(output.into(), command_action("true", false));
        first
            .insert_edge(edge(output, output, true, false))
            .expect("test graph output aliases must be unique");
    }
    let mut second = BuildGraph::default();
    for output in ["beta", "alpha"] {
        second
            .actions
            .insert(output.into(), command_action("true", false));
        second
            .insert_edge(edge(output, output, true, false))
            .expect("test graph output aliases must be unique");
    }

    assert_eq!(
        generate_posix(&first),
        generate_posix(&second),
        "the hidden identity must derive from the graph rather than insertion order"
    );
}

#[rstest]
fn distinct_phony_edges_get_distinct_hidden_outputs() {
    let mut graph = BuildGraph::default();
    graph
        .actions
        .insert("a".into(), command_action("true", false));
    for output in ["alpha", "beta"] {
        graph
            .insert_edge(edge("a", output, true, false))
            .expect("test graph output aliases must be unique");
    }

    let ninja = generate_posix(&graph);
    let hidden: Vec<String> = ninja
        .lines()
        .filter_map(|line| line.strip_prefix("build .netsuke/phony/edge-"))
        .filter_map(|rest| rest.split_once(':').map(|(digest, _)| digest.to_owned()))
        .collect();

    assert_eq!(
        hidden.len(),
        2,
        "each phony edge must emit exactly one hidden statement: {ninja}"
    );
    assert_ne!(
        hidden.first(),
        hidden.last(),
        "two edges must not share one hidden identity: {ninja}"
    );
}

#[rstest]
fn bundle_generation_matches_string_generation_for_forced_edges() {
    let graph = graph_of(command_action("true", false), edge("a", "out", true, false));

    let bundle = generate_bundle(&graph).expect("generate bundle");
    let string_only = generate(&graph).expect("generate string manifest");

    assert_eq!(
        bundle.build_file(),
        string_only,
        "both emission paths must place the force statement and alias identically"
    );
}

#[rstest]
fn the_phony_namespace_is_reserved_for_string_generation() {
    let edge = BuildEdge {
        explicit_outputs: vec![Utf8PathBuf::from(".netsuke/phony/edge-forged")],
        ..edge("a", "out", false, false)
    };
    let graph = graph_of(command_action("true", false), edge);

    let error = generate(&graph).expect_err("a user path inside .netsuke/phony must be rejected");

    assert!(
        matches!(error, NinjaGenError::ReservedOutputPath { .. }),
        "expected ReservedOutputPath, got {error:?}"
    );
}
