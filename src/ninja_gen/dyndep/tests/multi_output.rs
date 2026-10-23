//! Regression coverage for serial staging of a multi-output canonical edge.

use super::{graph_with_edge, serial_edge};
use crate::ninja_gen::dyndep::generate_bundle;
use anyhow::{Result, ensure};
use camino::Utf8PathBuf;

/// One multi-output serial edge must stage one gate set and render once.
///
/// The edge is indexed under two output aliases. Rendering walks the canonical
/// arena, so the shared edge must appear exactly once: a single build statement
/// listing every alias, and one gate per dependency rather than one gate set
/// per alias.
#[test]
fn multi_output_serial_edge_renders_one_statement_and_one_gate_set() -> Result<()> {
    let mut edge = serial_edge("all", &["check-fmt", "lint", "test"]);
    edge.explicit_outputs.push(Utf8PathBuf::from("extra"));
    let graph = graph_with_edge(edge);

    let bundle = generate_bundle(&graph)?;
    let file = bundle.build_file();

    ensure!(
        file.lines()
            .filter(|line| line.starts_with("build all extra: "))
            .count()
            == 1,
        "a multi-output edge must render one build statement: {file}"
    );
    ensure!(
        file.lines()
            .filter(|line| line.starts_with("build .netsuke/serial/"))
            .count()
            == 3,
        "one gate per dependency, not one gate set per output alias: {file}"
    );
    ensure!(
        bundle.dyndep_files().len() == 3,
        "expected one sidecar per dependency, got {}",
        bundle.dyndep_files().len()
    );
    Ok(())
}
