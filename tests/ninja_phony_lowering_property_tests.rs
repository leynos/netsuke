//! Properties over the `phony` and `always` lowerings.
//!
//! The fixed cases in `tests/ninja_phony_lowering_tests.rs` pin readable
//! statements one shape at a time. These properties generalise the invariants
//! those cases demonstrate, over graphs whose action names, output names,
//! commands, `restat` flags and `phony`/`always` combinations all vary:
//!
//! - byte output depends on the graph, not on the order the edges were
//!   inserted;
//! - two `phony` edges never share a hidden output;
//! - every executable `phony` edge and every non-phony `always` edge depends
//!   on the shared force node, and nothing else does;
//! - hidden statements never leave `restat = 1` in force, and a non-phony
//!   `always` edge carries the edge-level override unless its rule already
//!   declares the flag.

use camino::Utf8PathBuf;
use netsuke::ast::{CommandIsolation, Recipe};
use netsuke::ir::{Action, BuildEdge, BuildGraph, DependencyOrder, EdgeOutputVisibility};
use netsuke::ninja_gen::{RecipeShell, generate_with_shell};
use proptest::prelude::*;
use std::collections::BTreeSet;
use test_support::ninja_gen::paths_strategy;

/// The shared, permanently dirty prerequisite named by every forced edge.
const FORCE_PATH: &str = ".netsuke/phony/force";
/// The statement that declares the force prerequisite.
const FORCE_STATEMENT: &str = "build .netsuke/phony/force: phony";
/// Prefix of the hidden output that carries an executable `phony` edge.
const HIDDEN_PREFIX: &str = ".netsuke/phony/edge-";

/// One generated executable edge and the action that supplies its recipe.
///
/// The index makes declared outputs unique across the graph, so every
/// generated combination is a valid graph rather than a rejected duplicate.
#[derive(Clone, Debug)]
struct EdgeSpec {
    /// Position in the generated specification, used to keep names unique.
    index: usize,
    /// Lowercase stem shared by the action rule and the output name.
    name: String,
    /// Recipe text, rendered by the action's rule.
    command: String,
    /// Whether the action's rule declares `restat = 1`.
    restat: bool,
    /// Whether the edge lowers to a hidden statement behind a phony alias.
    phony: bool,
    /// Whether the edge gains the force node as a rendered dependency.
    always: bool,
    /// Declared inputs, which vary the statement header.
    inputs: Vec<Utf8PathBuf>,
}

impl EdgeSpec {
    /// Return the action id and Ninja rule name for this edge.
    fn action_id(&self) -> String {
        format!("{}{}", self.name, self.index)
    }

    /// Return the declared output name for this edge.
    ///
    /// The index suffix is what guarantees uniqueness: a Ninja edge may not
    /// declare an output another edge already owns.
    fn output(&self) -> Utf8PathBuf {
        Utf8PathBuf::from(format!("{}-out{}", self.name, self.index))
    }

    /// Return the statement header that introduces this edge's public name.
    fn public_header(&self) -> String {
        format!("build {}: ", self.output())
    }

    /// Return the statement header that introduces this edge's phony alias.
    fn alias_header(&self) -> String {
        format!("build {}: phony ", self.output())
    }
}

/// Build a strategy for a non-empty graph of executable edges.
///
/// Every dimension the lowering reads is generated: the action name, the
/// command, the rule-level `restat` flag, the `phony` and `always` flags, the
/// input list, and the number of edges. Outputs are derived from the
/// per-edge index instead, so the strategy cannot mint two edges that claim
/// the same output.
fn edge_specs() -> impl Strategy<Value = Vec<EdgeSpec>> {
    prop::collection::vec(
        (
            "[a-z][a-z0-9_]{0,6}",
            "[a-z]{1,6}",
            any::<bool>(),
            any::<bool>(),
            any::<bool>(),
            paths_strategy("in", 0..3),
        ),
        1..6,
    )
    .prop_map(|edges| {
        edges
            .into_iter()
            .enumerate()
            .map(
                |(index, (name, command, restat, phony, always, inputs))| EdgeSpec {
                    index,
                    name,
                    command,
                    restat,
                    phony,
                    always,
                    inputs,
                },
            )
            .collect()
    })
}

/// Build the action that carries one edge's recipe.
fn action_for(spec: &EdgeSpec) -> Action {
    Action {
        recipe: Recipe::Command {
            command: spec.command.clone().into(),
            isolation: CommandIsolation::Shared,
        },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: spec.restat,
    }
}

/// Build the edge described by `spec`.
fn edge_for(spec: &EdgeSpec) -> BuildEdge {
    BuildEdge {
        action_id: spec.action_id(),
        inputs: spec.inputs.clone(),
        implicit_deps: Vec::new(),
        dependency_order: DependencyOrder::Parallel,
        explicit_outputs: vec![spec.output()],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        progress_label: None,
        phony: spec.phony,
        always: spec.always,
        output_visibility: EdgeOutputVisibility::default(),
    }
}

/// Insert `specs` in the order the iterator yields them.
fn graph_from<'a>(specs: impl Iterator<Item = &'a EdgeSpec>) -> BuildGraph {
    let mut graph = BuildGraph::default();
    for spec in specs {
        graph.actions.insert(spec.action_id(), action_for(spec));
        graph
            .insert_edge(edge_for(spec))
            .expect("generated outputs are unique, so insertion cannot fail");
    }
    graph
}

/// Render a graph with explicit POSIX shell semantics.
fn ninja(graph: &BuildGraph) -> String {
    generate_with_shell(graph, RecipeShell::Posix).expect("generate POSIX Ninja manifest")
}

/// Collect the statement block beginning with `header`.
///
/// Ninja separates statements with a blank line, so a block is everything from
/// its header up to the next blank line. Reading a block rather than searching
/// the whole file keeps rule-level bindings out of edge-level assertions.
fn statement(ninja: &str, header: &str) -> Option<String> {
    let block: Vec<&str> = ninja
        .lines()
        .skip_while(|line| !line.starts_with(header))
        .take_while(|line| !line.is_empty())
        .collect();
    (!block.is_empty()).then(|| block.join("\n"))
}

/// Return the hidden outputs declared by a rendered graph, in emission order.
fn hidden_outputs(ninja: &str) -> Vec<String> {
    ninja
        .lines()
        .filter_map(|line| line.strip_prefix("build .netsuke/phony/edge-"))
        .filter_map(|rest| rest.split_once(':'))
        .map(|(digest, _)| format!("{HIDDEN_PREFIX}{digest}"))
        .collect()
}

/// Read the header line of a statement block.
fn header_line(block: &str) -> &str {
    block.lines().next().unwrap_or_default()
}

/// Read the hidden output a `phony` edge's alias republishes.
fn aliased_hidden(alias: &str) -> String {
    alias
        .split_once(": phony ")
        .map(|(_, hidden)| hidden.trim().to_owned())
        .expect("a phony alias names the hidden output it republishes")
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Byte output is a function of the graph, not of insertion order.
    ///
    /// Every cyclic rotation and the reversal of the generated edges are
    /// inserted into fresh graphs; all of them must render identically.
    #[test]
    fn generation_is_independent_of_insertion_order(specs in edge_specs()) {
        let expected = ninja(&graph_from(specs.iter()));
        let count = specs.len();
        for shift in 1..count {
            let rotated = specs.iter().cycle().skip(shift).take(count);
            prop_assert_eq!(
                ninja(&graph_from(rotated)),
                expected.clone(),
                "rotation by {} of {} edges changed the manifest",
                shift,
                count,
            );
        }
        prop_assert_eq!(
            ninja(&graph_from(specs.iter().rev())),
            expected,
            "reversing {} edges changed the manifest",
            count,
        );
    }

    /// Two `phony` edges never share a hidden output.
    #[test]
    fn distinct_phony_edges_get_distinct_hidden_outputs(specs in edge_specs()) {
        let rendered = ninja(&graph_from(specs.iter()));
        let hidden = hidden_outputs(&rendered);
        let phony_edges = specs.iter().filter(|spec| spec.phony).count();
        prop_assert_eq!(
            hidden.len(),
            phony_edges,
            "each phony edge must emit one hidden statement: {}",
            rendered,
        );
        let unique: BTreeSet<&String> = hidden.iter().collect();
        prop_assert_eq!(
            unique.len(),
            hidden.len(),
            "hidden identities must be distinct: {:?}",
            hidden,
        );
        for output in &hidden {
            prop_assert!(
                output.starts_with(HIDDEN_PREFIX),
                "a hidden output must live under the reserved namespace: {output}",
            );
        }
    }

    /// Forced edges depend on the shared force node, and nothing else does.
    #[test]
    fn forced_edges_depend_on_the_shared_force_node(specs in edge_specs()) {
        let rendered = ninja(&graph_from(specs.iter()));
        let forces_something = specs.iter().any(|spec| spec.phony || spec.always);
        prop_assert_eq!(
            rendered.matches(FORCE_STATEMENT).count(),
            usize::from(forces_something),
            "the force statement must be emitted exactly when an edge consumes it: {}",
            rendered,
        );
        for spec in &specs {
            let dependency = format!("| {FORCE_PATH}");
            if spec.phony {
                let alias = statement(&rendered, &spec.alias_header())
                    .expect("a phony edge must publish an alias");
                let hidden = aliased_hidden(&alias);
                let block = statement(&rendered, &format!("build {hidden}: "))
                    .expect("a phony edge must carry its recipe on a hidden statement");
                prop_assert!(
                    header_line(&block).contains(&dependency),
                    "a phony edge's recipe must depend on the force node: {block}",
                );
            } else {
                let block = statement(&rendered, &spec.public_header())
                    .expect("a non-phony edge declares its own statement");
                prop_assert_eq!(
                    header_line(&block).contains(&dependency),
                    spec.always,
                    "only an always edge gains the force node: {}",
                    block,
                );
            }
        }
    }

    /// `restat` follows the lowering's suppression and retention rules.
    #[test]
    fn restat_follows_the_lowering(specs in edge_specs()) {
        let rendered = ninja(&graph_from(specs.iter()));
        for spec in &specs {
            if spec.phony {
                let alias = statement(&rendered, &spec.alias_header())
                    .expect("a phony edge must publish an alias");
                let hidden = aliased_hidden(&alias);
                let block = statement(&rendered, &format!("build {hidden}: "))
                    .expect("a phony edge must carry its recipe on a hidden statement");
                prop_assert!(
                    !block.lines().any(|line| line.trim() == "restat = 1"),
                    "a never-created hidden output must not keep restat = 1: {block}",
                );
                let cleared = block.lines().any(|line| line.trim_end() == "  restat =");
                prop_assert_eq!(
                    cleared,
                    spec.restat,
                    "an inherited rule-level restat must be cleared exactly when the rule sets it: {}",
                    block,
                );
            } else {
                let block = statement(&rendered, &spec.public_header())
                    .expect("a non-phony edge declares its own statement");
                prop_assert_eq!(
                    block.contains("  restat = 1"),
                    spec.always && !spec.restat,
                    "an always edge overrides restat only when its rule does not declare it: {}",
                    block,
                );
            }
        }
    }
}
