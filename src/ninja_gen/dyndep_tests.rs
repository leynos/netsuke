//! Unit tests for staged dyndep bundle generation.

use super::*;
use crate::ast::Recipe;
use crate::ir::{Action, BuildGraph};
use anyhow::{Context, Result, ensure};
use camino::Utf8PathBuf;

fn action(command: &str) -> Action {
    Action {
        recipe: Recipe::Command {
            command: command.into(),
        },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    }
}

fn serial_edge(output: &str, deps: &[&str]) -> BuildEdge {
    let implicit_deps: Vec<_> = deps.iter().map(|d| Utf8PathBuf::from(d)).collect();
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
    graph.targets.insert(edge.explicit_outputs[0].clone(), edge);
    graph
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
    for dep in ["check-fmt", "lint", "test"] {
        let revealed = bundle
            .dyndep_files()
            .iter()
            .any(|dd| dd.content().contains(dep));
        ensure!(revealed, "expected a sidecar revealing {dep}");
    }
    let file = bundle.build_file();
    ensure!(
        file.lines()
            .filter(|l| l.starts_with("build .netsuke/serial/"))
            .count()
            == 3,
        "expected three gate edges"
    );
    ensure!(
        file.contains("build all: a |"),
        "aggregate edge must list gates as implicit deps"
    );
    Ok(())
}

#[test]
fn serial_sidecars_reveal_real_deps_in_order() -> Result<()> {
    let graph = graph_with_edge(serial_edge("all", &["check-fmt", "lint", "test"]));
    let bundle = generate_bundle(&graph)?;
    let contents: Vec<&str> = bundle.dyndep_files().iter().map(|d| d.content()).collect();
    // Sidecar order follows declaration order because the first sidecar has no
    // predecessor while later sidecars are produced by ordered edges.
    let fmt_at = contents
        .iter()
        .position(|c| c.contains("check-fmt"))
        .context("check-fmt sidecar missing")?;
    let lint_at = contents
        .iter()
        .position(|c| c.contains("lint"))
        .context("lint sidecar missing")?;
    let test_at = contents
        .iter()
        .position(|c| c.contains("test"))
        .context("test sidecar missing")?;
    ensure!(
        fmt_at < lint_at && lint_at < test_at,
        "sidecars must preserve declaration order"
    );
    Ok(())
}

#[test]
fn parallel_edges_produce_no_sidecars() -> Result<()> {
    let graph = graph_with_edge(parallel_edge("all", &["dep1", "dep2"]));
    let bundle = generate_bundle(&graph)?;
    ensure!(
        !bundle.build_file().contains("ninja_required_version"),
        "parallel bundle must not emit a version floor"
    );
    ensure!(
        bundle.dyndep_files().is_empty(),
        "parallel graph must produce no sidecars"
    );
    Ok(())
}

#[test]
fn one_element_serial_list_needs_no_gates() -> Result<()> {
    let graph = graph_with_edge(serial_edge("all", &["dep1"]));
    let bundle = generate_bundle(&graph)?;
    ensure!(
        !bundle.build_file().contains("ninja_required_version"),
        "single-dependency serial list must not emit a version floor"
    );
    ensure!(
        bundle.dyndep_files().is_empty(),
        "single-dependency serial list must produce no sidecars"
    );
    Ok(())
}

#[test]
fn repeated_dependency_keeps_separate_stage_sidecars() -> Result<()> {
    let graph = graph_with_edge(serial_edge("all", &["same", "same"]));
    let bundle = generate_bundle(&graph)?;
    // Each gate stage is distinct, so each stage has its own content-addressed
    // sidecar even when the revealed dependency is the same node. Ninja
    // unifies the real dependency path, so its recipe still runs once.
    ensure!(
        bundle
            .build_file()
            .lines()
            .filter(|l| l.starts_with("build .netsuke/serial/"))
            .count()
            == 2,
        "expected two gate edges for a repeated dependency"
    );
    ensure!(
        bundle.dyndep_files().len() == 2,
        "each stage needs its own sidecar, got {}",
        bundle.dyndep_files().len()
    );
    let contents: Vec<&str> = bundle.dyndep_files().iter().map(|d| d.content()).collect();
    ensure!(
        contents.iter().all(|c| c.contains("same")),
        "every stage sidecar must reveal the shared dependency"
    );
    Ok(())
}

#[test]
fn reserved_output_namespace_is_rejected() -> Result<()> {
    let mut edge = parallel_edge("all", &["dep"]);
    edge.explicit_outputs = vec![Utf8PathBuf::from(".netsuke/serial/x")];
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
