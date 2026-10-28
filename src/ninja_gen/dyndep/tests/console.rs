//! Tests for console selection across staged serial dependency bundles.

use super::{generate_bundle, graph_with_edge, serial_edge};
use crate::ir::{BuildEdge, DependencyOrder};
use anyhow::{Result, ensure};
use camino::Utf8PathBuf;

/// Keep console scheduling on the user edge, away from synthetic gates and sidecars.
#[test]
fn console_serial_parent_does_not_mark_gates_or_sidecars() -> Result<()> {
    let mut edge = serial_edge("all", &["check-fmt", "lint"]);
    edge.console = true;
    let graph = graph_with_edge(edge);
    let bundle = generate_bundle(&graph)?;
    ensure!(
        bundle.console_edge_count() == 1,
        "console telemetry must count the user edge without synthetic gates or sidecars"
    );
    let file = bundle.build_file();
    let lines: Vec<_> = file.lines().collect();
    let parent_line = lines
        .iter()
        .position(|line| line.starts_with("build all: a |"))
        .expect("console parent edge must be emitted");

    ensure!(
        lines.get(parent_line + 1) == Some(&"  pool = console"),
        "console binding must follow the parent build statement:\n{file}"
    );
    ensure!(
        lines
            .iter()
            .filter(|line| **line == "  pool = console")
            .count()
            == 1,
        "synthetic gate edges must not select the console pool:\n{file}"
    );
    ensure!(
        file.lines()
            .filter(|line| line.starts_with("build .netsuke/serial/"))
            .count()
            == 2,
        "expected two synthetic serial gates:\n{file}"
    );
    ensure!(
        bundle
            .dyndep_files()
            .iter()
            .all(|sidecar| !sidecar.content().contains("console")),
        "dyndep sidecars must not contain console scheduling metadata"
    );
    Ok(())
}

/// Clear a shared console rule pool from ordinary edges in a dyndep bundle.
#[test]
fn ordinary_dyndep_edge_clears_a_shared_console_action_pool() -> Result<()> {
    let mut selected = serial_edge("all", &["check-fmt", "lint"]);
    selected.console = true;
    let mut graph = graph_with_edge(selected);
    graph.actions.get_mut("a").expect("test action exists").pool = Some("console".into());
    graph
        .insert_edge(BuildEdge {
            action_id: "a".into(),
            inputs: Vec::new(),
            implicit_deps: Vec::new(),
            dependency_order: DependencyOrder::Parallel,
            explicit_outputs: vec![Utf8PathBuf::from("ordinary")],
            implicit_outputs: Vec::new(),
            order_only_deps: Vec::new(),
            progress_label: None,
            phony: false,
            always: false,
            console: false,
        })
        .expect("test graph output aliases must be unique");

    let bundle = generate_bundle(&graph)?;
    let file = bundle.build_file();
    let lines: Vec<_> = file.lines().collect();
    let selected_index = lines
        .iter()
        .position(|line| line.starts_with("build all: a |"))
        .expect("selected serial edge must be emitted");
    ensure!(
        lines
            .iter()
            .skip(selected_index + 1)
            .take_while(|line| line.starts_with("  "))
            .any(|line| *line == "  pool = console"),
        "selected serial edge must retain console scheduling:\n{file}"
    );
    ensure!(
        file.contains("build ordinary: a\n  pool =\n\n"),
        "ordinary edge must clear the action rule's console pool:\n{file}"
    );
    let synthetic_edge_selects_console = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.starts_with("build .netsuke/serial/"))
        .any(|(index, _)| {
            lines
                .iter()
                .skip(index + 1)
                .take_while(|line| line.starts_with("  "))
                .any(|line| *line == "  pool = console")
        });
    ensure!(
        !synthetic_edge_selects_console,
        "synthetic serial gates must not select the console pool:\n{file}"
    );
    Ok(())
}
