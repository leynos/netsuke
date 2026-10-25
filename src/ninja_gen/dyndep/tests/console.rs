//! Tests for console selection across staged serial dependency bundles.

use super::{generate_bundle, graph_with_edge, serial_edge};
use anyhow::{Result, ensure};

/// Keep console scheduling on the user edge, away from synthetic gates and sidecars.
#[test]
fn console_serial_parent_does_not_mark_gates_or_sidecars() -> Result<()> {
    let mut edge = serial_edge("all", &["check-fmt", "lint"]);
    edge.console = true;
    let graph = graph_with_edge(edge);
    let bundle = generate_bundle(&graph)?;
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
