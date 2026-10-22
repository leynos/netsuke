//! Unit tests for staged dyndep bundle generation.

use super::{BuildEdge, GeneratedDyndep, NinjaGenError, generate_bundle};
use crate::ast::{Recipe, StringOrList};
use crate::ir::{Action, BuildGraph, DependencyOrder};
use anyhow::{Context, Result, ensure};
use camino::Utf8PathBuf;
use rstest::rstest;

fn action(command: &str) -> Action {
    Action {
        recipe: Recipe::Command {
            command: command.into(),
            isolation: crate::ast::CommandIsolation::Shared,
        },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    }
}

fn serial_edge(output: &str, deps: &[&str]) -> BuildEdge {
    let implicit_deps: Vec<_> = deps.iter().map(Utf8PathBuf::from).collect();
    BuildEdge {
        action_id: "a".into(),
        inputs: Vec::new(),
        implicit_deps,
        dependency_order: DependencyOrder::Serial,
        explicit_outputs: vec![Utf8PathBuf::from(output)],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    }
}

fn parallel_edge(output: &str, deps: &[&str]) -> BuildEdge {
    let mut edge = serial_edge(output, deps);
    edge.dependency_order = DependencyOrder::Parallel;
    edge
}

fn graph_with_edge(edge: BuildEdge) -> BuildGraph {
    let mut graph = BuildGraph::default();
    graph.actions.insert("a".into(), action("echo done"));
    assert!(
        graph.insert_edge(edge).is_ok(),
        "test graph output aliases must be unique",
    );
    graph
}

fn assert_edge_produces_no_staging(edge: BuildEdge) -> Result<()> {
    let graph = graph_with_edge(edge);
    let bundle = generate_bundle(&graph)?;
    ensure!(
        !bundle.build_file().contains("ninja_required_version"),
        "edge must not emit a version floor"
    );
    ensure!(
        bundle.dyndep_files().is_empty(),
        "edge must not produce sidecars"
    );
    Ok(())
}

#[test]
fn serial_bundle_emits_version_and_staged_sidecars() -> Result<()> {
    let graph = graph_with_edge(serial_edge("all", &["check-fmt", "lint", "test"]));
    let bundle = generate_bundle(&graph)?;
    ensure!(
        bundle
            .build_file()
            .contains("ninja_required_version = 1.10"),
        "serial bundle must declare the Ninja version floor"
    );
    ensure!(
        bundle.dyndep_files().len() == 3,
        "expected one sidecar per dependency, got {}",
        bundle.dyndep_files().len()
    );
    let file = bundle.build_file();
    ensure!(
        serial_paths(file, is_gate).len() == 3,
        "expected three gate edges, got {:?}",
        serial_paths(file, is_gate)
    );
    ensure!(
        serial_paths(file, is_relay).len() == 3,
        "expected one relay per dependency, got {:?}",
        serial_paths(file, is_relay)
    );
    Ok(())
}

#[test]
fn serial_sidecars_reveal_relays_that_carry_the_real_deps() -> Result<()> {
    let graph = graph_with_edge(serial_edge("all", &["check-fmt", "lint", "test"]));
    let bundle = generate_bundle(&graph)?;
    let relays = serial_paths(bundle.build_file(), is_relay);
    ensure!(relays.len() == 3, "expected three relays, got {relays:?}");
    // Each sidecar must reveal a relay, never the real dependency: revealing
    // the real path is the shape that reaches the upstream Ninja crash.
    for (index, relay) in relays.iter().enumerate() {
        let revealed = bundle
            .dyndep_files()
            .iter()
            .filter(|dd| dd.content().contains(relay.as_str()))
            .count();
        ensure!(
            revealed == 1,
            "relay {relay} (stage {index}) must be the revealed path of exactly \
             one sidecar, was revealed by {revealed}"
        );
    }
    for dep in ["check-fmt", "lint", "test"] {
        let leaks = bundle
            .dyndep_files()
            .iter()
            .filter(|dd| dd.content().contains(dep))
            .count();
        ensure!(
            leaks == 0,
            "the real dependency {dep} must not appear in any sidecar, found \
             {leaks}"
        );
    }
    Ok(())
}

#[test]
fn serial_sidecars_preserve_declaration_order() -> Result<()> {
    let graph = graph_with_edge(serial_edge("all", &["check-fmt", "lint", "test"]));
    let bundle = generate_bundle(&graph)?;
    let contents: Vec<&str> = bundle
        .dyndep_files()
        .iter()
        .map(GeneratedDyndep::content)
        .collect();
    // Sidecar order follows declaration order because the first sidecar has no
    // predecessor while later sidecars are produced by ordered edges.
    let position = |needle: &str| -> Result<usize> {
        contents
            .iter()
            .position(|content| content.contains(needle))
            .with_context(|| format!("{needle} sidecar missing: {contents:?}"))
    };
    let (fmt_at, lint_at, test_at) = (
        position("relay-000")?,
        position("relay-001")?,
        position("relay-002")?,
    );
    ensure!(
        fmt_at < lint_at && lint_at < test_at,
        "sidecars must preserve declaration order, got {fmt_at} {lint_at} {test_at}"
    );
    Ok(())
}

/// Every serial-namespace path a `build` line declares as an output.
///
/// `predicate` selects gates or relays, so a count cannot silently absorb the
/// other kind: a change that drops every relay would leave the gate count
/// intact and fail the relay count instead.
fn serial_paths(file: &str, predicate: fn(&str) -> bool) -> Vec<String> {
    file.lines()
        .filter_map(|line| line.strip_prefix("build "))
        .filter_map(|rest| rest.split_once(": ").map(|(lhs, _)| lhs))
        .flat_map(str::split_whitespace)
        .filter(|path| path.starts_with(".netsuke/serial/"))
        .filter(|path| predicate(path))
        .map(str::to_owned)
        .collect()
}

/// Whether a serial-namespace path names a gate rather than a relay.
fn is_gate(path: &str) -> bool {
    !is_relay(path)
}

/// Whether a serial-namespace path names a stage's relay phony.
fn is_relay(path: &str) -> bool {
    path.rsplit_once('/')
        .is_some_and(|(_, name)| name.starts_with("relay-"))
}

#[test]
fn parallel_edges_produce_no_sidecars() -> Result<()> {
    assert_edge_produces_no_staging(parallel_edge("all", &["dep1", "dep2"]))
}

#[test]
fn one_element_serial_list_needs_no_gates() -> Result<()> {
    assert_edge_produces_no_staging(serial_edge("all", &["dep1"]))
}

#[test]
fn parallel_bundle_matches_string_generation() -> Result<()> {
    let graph = graph_with_edge(parallel_edge("all", &["dep1", "dep2"]));
    let bundle = generate_bundle(&graph)?;
    ensure!(
        bundle.build_file() == crate::ninja_gen::generate(&graph)?,
        "parallel bundle output must match string generation"
    );
    Ok(())
}

#[test]
fn bundle_rejects_an_empty_command_list() -> Result<()> {
    let mut graph = graph_with_edge(parallel_edge("all", &[]));
    graph
        .actions
        .get_mut("a")
        .context("test graph must contain its action")?
        .recipe = Recipe::Command {
        command: StringOrList::List(Vec::new()),
        isolation: crate::ast::CommandIsolation::Shared,
    };

    let error = generate_bundle(&graph)
        .err()
        .context("bundle generation must reject an empty command list")?;
    ensure!(
        matches!(error, NinjaGenError::EmptyCommandRecipe { action_index: 1 }),
        "unexpected bundle-generation error: {error:?}"
    );
    Ok(())
}
#[test]
fn repeated_dependency_keeps_separate_stage_sidecars() -> Result<()> {
    let graph = graph_with_edge(serial_edge("all", &["same", "same"]));
    let bundle = generate_bundle(&graph)?;
    // Each gate stage is distinct, so each stage has its own content-addressed
    // sidecar even when the relay carries the same dependency. Ninja unifies
    // the real dependency path, so its recipe still runs once.
    let file = bundle.build_file();
    ensure!(
        serial_paths(file, is_gate).len() == 2,
        "expected two gate edges for a repeated dependency, got {:?}",
        serial_paths(file, is_gate)
    );
    ensure!(
        serial_paths(file, is_relay).len() == 2,
        "each stage needs its own relay, got {:?}",
        serial_paths(file, is_relay)
    );
    ensure!(
        bundle.dyndep_files().len() == 2,
        "each stage needs its own sidecar, got {}",
        bundle.dyndep_files().len()
    );
    let relay_lines: Vec<&str> = file.lines().filter(|line| is_relay(line)).collect();
    ensure!(
        relay_lines.iter().all(|line| line.ends_with("| same")),
        "every relay must carry the shared dependency, got {relay_lines:?}"
    );
    let contents: Vec<&str> = bundle
        .dyndep_files()
        .iter()
        .map(GeneratedDyndep::content)
        .collect();
    ensure!(
        contents.iter().all(|c| !c.contains("| same")),
        "no sidecar may reveal the shared real dependency, got {contents:?}"
    );
    Ok(())
}

#[derive(Clone, Copy)]
enum EdgePathField {
    ExplicitOutput,
    ImplicitOutput,
    Input,
    ImplicitDependency,
    OrderOnlyDependency,
}

#[derive(Clone, Copy)]
enum GraphPathField {
    Edge(EdgePathField),
    DefaultTarget,
}

fn set_edge_path(edge: &mut BuildEdge, field: EdgePathField, path: &str) {
    let paths = vec![Utf8PathBuf::from(path)];
    match field {
        EdgePathField::ExplicitOutput => edge.explicit_outputs = paths,
        EdgePathField::ImplicitOutput => edge.implicit_outputs = paths,
        EdgePathField::Input => edge.inputs = paths,
        EdgePathField::ImplicitDependency => edge.implicit_deps = paths,
        EdgePathField::OrderOnlyDependency => edge.order_only_deps = paths,
    }
}

fn graph_with_path(field: GraphPathField, path: &str) -> BuildGraph {
    let mut edge = parallel_edge("all", &["dep"]);
    if let GraphPathField::Edge(edge_field) = field {
        set_edge_path(&mut edge, edge_field, path);
        if matches!(edge_field, EdgePathField::ExplicitOutput) {
            edge.explicit_outputs.insert(0, Utf8PathBuf::from("all"));
        }
    }
    let mut graph = graph_with_edge(edge);
    if matches!(field, GraphPathField::DefaultTarget) {
        graph.default_targets = vec![Utf8PathBuf::from(path)];
    }
    graph
}

#[rstest]
fn unsupported_control_characters_are_rejected_in_every_path_field(
    #[values(
        GraphPathField::Edge(EdgePathField::ExplicitOutput),
        GraphPathField::Edge(EdgePathField::ImplicitOutput),
        GraphPathField::Edge(EdgePathField::Input),
        GraphPathField::Edge(EdgePathField::ImplicitDependency),
        GraphPathField::Edge(EdgePathField::OrderOnlyDependency),
        GraphPathField::DefaultTarget
    )]
    field: GraphPathField,
    #[values('\0', '\t', '\r', '\n')] character: char,
) -> Result<()> {
    let path = format!("invalid{character}path");
    let graph = graph_with_path(field, &path);
    let error = generate_bundle(&graph)
        .err()
        .context("unsupported control character must be rejected")?;
    ensure!(
        matches!(
            error,
            NinjaGenError::UnsupportedPathCharacter {
                character: actual,
                ..
            } if actual == character
        ),
        "expected UnsupportedPathCharacter for {character:?}, got {error:?}"
    );
    Ok(())
}

#[rstest]
#[case::dollar("path$money", '$')]
#[case::colon("path:colon", ':')]
fn ninja_metacharacters_are_rejected_in_every_path_field(
    #[values(
        GraphPathField::Edge(EdgePathField::ExplicitOutput),
        GraphPathField::Edge(EdgePathField::ImplicitOutput),
        GraphPathField::Edge(EdgePathField::Input),
        GraphPathField::Edge(EdgePathField::ImplicitDependency),
        GraphPathField::Edge(EdgePathField::OrderOnlyDependency),
        GraphPathField::DefaultTarget
    )]
    field: GraphPathField,
    #[case] path: &str,
    #[case] character: char,
) -> Result<()> {
    let graph = graph_with_path(field, path);
    let generated_error = crate::ninja_gen::generate(&graph)
        .err()
        .context("Ninja metacharacter must be rejected during string generation")?;
    let bundle_error = generate_bundle(&graph)
        .err()
        .context("Ninja metacharacter must be rejected during bundle generation")?;

    ensure!(
        matches!(
            generated_error,
            NinjaGenError::UnsupportedPathCharacter {
                character: actual,
                ..
            } if actual == character
        ),
        "string generation must reject {character:?}, got {generated_error:?}"
    );
    ensure!(
        matches!(
            bundle_error,
            NinjaGenError::UnsupportedPathCharacter {
                character: actual,
                ..
            } if actual == character
        ),
        "bundle generation must reject {character:?}, got {bundle_error:?}"
    );
    Ok(())
}

/// Render whitespace in every graph path field with Ninja's literal-space escape.
#[rstest]
fn whitespace_paths_are_escaped_in_every_path_field(
    #[values(
        GraphPathField::Edge(EdgePathField::ExplicitOutput),
        GraphPathField::Edge(EdgePathField::ImplicitOutput),
        GraphPathField::Edge(EdgePathField::Input),
        GraphPathField::Edge(EdgePathField::ImplicitDependency),
        GraphPathField::Edge(EdgePathField::OrderOnlyDependency),
        GraphPathField::DefaultTarget
    )]
    field: GraphPathField,
) -> Result<()> {
    let graph = graph_with_path(field, "path with space");
    let generated = crate::ninja_gen::generate(&graph)?;
    let bundle = generate_bundle(&graph)?;
    ensure!(generated.contains("path$ with$ space"));
    ensure!(bundle.build_file().contains("path$ with$ space"));
    Ok(())
}
#[rstest]
#[case::explicit_output(EdgePathField::ExplicitOutput)]
#[case::implicit_output(EdgePathField::ImplicitOutput)]
#[case::input(EdgePathField::Input)]
#[case::implicit_dependency(EdgePathField::ImplicitDependency)]
#[case::order_only_dependency(EdgePathField::OrderOnlyDependency)]
fn reserved_output_namespace_is_rejected(#[case] field: EdgePathField) -> Result<()> {
    let mut edge = parallel_edge("all", &["dep"]);
    set_edge_path(&mut edge, field, ".netsuke/serial/x");
    if edge.explicit_outputs == [Utf8PathBuf::from(".netsuke/serial/x")] {
        edge.explicit_outputs.insert(0, Utf8PathBuf::from("all"));
    }
    let graph = graph_with_edge(edge);
    let err = generate_bundle(&graph)
        .err()
        .context("reserved path must be rejected")?;
    ensure!(
        matches!(err, NinjaGenError::ReservedOutputPath { .. }),
        "expected ReservedOutputPath, got {err:?}"
    );
    Ok(())
}

#[test]
fn similarly_prefixed_namespace_is_accepted() -> Result<()> {
    let mut edge = parallel_edge("all", &["dep"]);
    edge.implicit_outputs = vec![Utf8PathBuf::from(".netsuke-extra/x")];
    generate_bundle(&graph_with_edge(edge))?;
    Ok(())
}

#[test]
fn pipe_in_path_is_rejected_before_generation() -> Result<()> {
    let graph = graph_with_edge(serial_edge("all", &["unsupported|dependency", "test"]));
    let err = generate_bundle(&graph)
        .err()
        .context("pipe path must be rejected")?;
    ensure!(
        matches!(
            err,
            NinjaGenError::UnsupportedPathCharacter { character: '|', .. }
        ),
        "expected UnsupportedPathCharacter, got {err:?}"
    );
    Ok(())
}

mod dependency_only;

mod multi_output;
