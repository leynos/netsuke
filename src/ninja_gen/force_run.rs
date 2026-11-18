//! Name the shared force node and hidden outputs for always and phony edges.
//!
//! Ninja decides freshness from files. An executable `phony` edge therefore
//! needs an output name no user file can shadow, and an `always` edge needs a
//! prerequisite that is permanently out of date. Both lowerings share the
//! reserved `.netsuke/phony` namespace: the force node is one no-input `phony`
//! statement, and each executable `phony` edge owns a content-addressed hidden
//! output derived from its declared outputs.
//!
//! Keeping the naming here leaves the edge writer responsible only for the
//! Ninja statement shape. The identity is derived from the graph, so byte
//! output stays a function of the graph rather than of emission order.

use super::dyndep::content_digest;
use crate::ir::{BuildEdge, BuildGraph};
use camino::Utf8PathBuf;
use std::fmt::Write;

/// Reserved namespace for generated phony and always state.
pub(super) const PHONY_NAMESPACE: &str = ".netsuke/phony";
/// Permanently dirty prerequisite shared by every forced edge.
pub(super) const FORCE_PATH: &str = ".netsuke/phony/force";
/// Schema tag incorporated into every hidden-output identity.
const PHONY_SCHEMA: &str = "netsuke-phony-v1";
/// Prefix distinguishing a hidden output from the force node.
const HIDDEN_PREFIX: &str = "edge-";
/// Separator between paths inside one output group. Paths cannot contain it.
const GROUP_SEPARATOR: char = '\u{1}';

/// Return whether any rendered edge consumes the shared force node.
///
/// Checked once per generation so a graph that needs no force node keeps its
/// previous byte output.
pub(super) fn graph_requires_force(graph: &BuildGraph) -> bool {
    graph.edges().any(|edge| {
        graph
            .actions
            .get(&edge.action_id)
            .is_some_and(|action| edge_needs_force(edge, action.recipe.is_dependency_only()))
    })
}

/// Return whether one edge must depend on the shared force node.
///
/// Dependency-only actions execute nothing, so forcing them would only add a
/// statement with no command behind it.
pub(super) const fn edge_needs_force(edge: &BuildEdge, is_dependency_only: bool) -> bool {
    !is_dependency_only && (edge.phony || edge.always)
}

/// Write the shared no-input force statement when the graph consumes it.
///
/// Emitted at the start of the build-statement section so the position does
/// not depend on which edge first needs it.
///
/// # Errors
///
/// Returns a formatting error when the writer fails.
pub(super) fn write_force_statement<W: Write>(graph: &BuildGraph, out: &mut W) -> std::fmt::Result {
    if graph_requires_force(graph) {
        writeln!(out, "build {FORCE_PATH}: phony")?;
        writeln!(out)?;
    }
    Ok(())
}

/// Derive the hidden output that carries an executable `phony` edge's command.
///
/// The identity covers the explicit and implicit outputs so two edges cannot
/// share a hidden name, and the `edge-` prefix keeps the force node from ever
/// becoming a parent directory of a hidden output.
pub(super) fn hidden_output(edge: &BuildEdge) -> Utf8PathBuf {
    let mut canonical = String::new();
    push_output_group(&mut canonical, &edge.explicit_outputs);
    canonical.push(GROUP_SEPARATOR);
    push_output_group(&mut canonical, &edge.implicit_outputs);
    let digest = content_digest(PHONY_SCHEMA, &canonical);
    Utf8PathBuf::from(format!("{PHONY_NAMESPACE}/{HIDDEN_PREFIX}{digest}"))
}

/// Append one NUL-terminated output group to the hidden-output identity.
fn push_output_group(canonical: &mut String, outputs: &[Utf8PathBuf]) {
    for output in outputs {
        canonical.push_str(output.as_str());
        // A NUL both terminates the path and separates it from the next one.
        canonical.push('\0');
    }
}
