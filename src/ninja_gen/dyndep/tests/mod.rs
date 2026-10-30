//! Unit tests for staged dyndep bundle generation.

use super::{GeneratedDyndep, NinjaGenError, generate_bundle};
use crate::ast::{Recipe, StringOrList};
use crate::ir::{Action, BuildEdge, BuildGraph, DependencyOrder};
use anyhow::{Context, Result, ensure};
use camino::Utf8PathBuf;

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
        progress_label: None,
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

/// Every `build` line declaring a serial-namespace output matching `predicate`.
///
/// [`serial_paths`] answers *which paths* were declared; a caller that needs to
/// read a line's dependency tail needs the lines themselves. Both select on the
/// left-hand side only, so a dependency containing `/` cannot be mistaken for
/// part of an output name — the reason a test must never hand a whole `build`
/// line to [`is_relay`], which splits at the last `/`.
fn serial_build_lines(file: &str, predicate: fn(&str) -> bool) -> Vec<&str> {
    file.lines()
        .filter(|line| {
            line.strip_prefix("build ")
                .and_then(|rest| rest.split_once(": "))
                .is_some_and(|(lhs, _)| {
                    lhs.split_whitespace()
                        .any(|path| path.starts_with(".netsuke/serial/") && predicate(path))
                })
        })
        .collect()
}

/// Whether a serial-namespace path names a gate rather than a relay.
fn is_gate(path: &str) -> bool {
    !is_relay(path)
}

/// Whether a serial-namespace path names a stage's relay phony.
///
/// The final component is taken on *either* separator, because `staging` builds
/// the relay with `Utf8PathBuf::join` and that applies the host's separator: a
/// Unix run renders `.netsuke/serial/<digest>/relay-000` while a Windows run
/// renders `.netsuke/serial/<digest>\relay-000`. Splitting on `/` alone read
/// every Windows relay as a gate, so the Windows lane counted six gates where
/// the bundle declares three. See
/// [`relay_and_gate_names_are_classified_on_both_separators`] for the
/// host-independent regression coverage.
///
/// Accepting both separators cannot misclassify a real emitted path: the
/// emitter writes this namespace itself, so the segment before the relay name
/// is always a lower-hex digest, and the reserved-namespace check rejects a
/// user path that tries to reach it.
fn is_relay(path: &str) -> bool {
    path.rsplit(['/', '\\'])
        .next()
        .is_some_and(|name| name.starts_with("relay-"))
}

/// Pairs of gate and relay names differing only in path separator.
///
/// The emitter joins with the host separator, so a separator test that ran
/// only on Unix would pass while the Windows lane failed — which is exactly
/// what happened before [`is_relay`] learned to split on both. These literals
/// let one Unix run cover the Windows shape.
#[derive(Clone, Copy)]
struct RelayNames {
    /// A gate path in the emitted spelling for one platform.
    gate: &'static str,
    /// That platform's relay path for the same stage.
    relay: &'static str,
}

#[test]
fn relay_and_gate_names_are_classified_on_both_separators() -> Result<()> {
    let unix = RelayNames {
        gate: ".netsuke/serial/abc/000",
        relay: ".netsuke/serial/abc/relay-000",
    };
    let windows = RelayNames {
        gate: r".netsuke\serial\abc\000",
        relay: r".netsuke\serial\abc\relay-000",
    };
    for names in [unix, windows] {
        ensure!(
            is_relay(names.relay),
            "{} must be classified as a relay",
            names.relay
        );
        ensure!(
            is_gate(names.gate),
            "{} must be classified as a gate, not a relay",
            names.gate
        );
        ensure!(
            !is_relay(names.gate),
            "a gate must not be classified as a relay: {}",
            names.gate
        );
    }
    Ok(())
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
    let relay_lines = serial_build_lines(file, is_relay);
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

mod dependency_only;

mod multi_output;

mod paths;

mod serial_snapshot;
