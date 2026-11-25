//! Shared graph builders for the cycle module's unit tests.
//!
//! Keeping edge construction in one place lets the focused detector tests and
//! the alias-amplification regressions describe only the property they check.

use camino::Utf8PathBuf;

use super::super::graph::BuildEdge;

/// Convert a test name into the graph's UTF-8 path representation.
pub(super) fn path(name: &str) -> Utf8PathBuf {
    Utf8PathBuf::from(name)
}

/// Build a single-output edge from its dependency lists.
pub(super) fn build_edge(inputs: &[&str], implicit_deps: &[&str], output: &str) -> BuildEdge {
    build_multi_output_edge(&[output], &[], inputs, implicit_deps)
}

/// Build an edge with explicit and implicit output aliases.
pub(super) fn build_multi_output_edge(
    explicit_outputs: &[&str],
    implicit_outputs: &[&str],
    inputs: &[&str],
    implicit_deps: &[&str],
) -> BuildEdge {
    BuildEdge {
        action_id: "id".into(),
        inputs: inputs.iter().map(|name| path(name)).collect(),
        implicit_deps: implicit_deps.iter().map(|name| path(name)).collect(),
        dependency_order: crate::ir::DependencyOrder::Parallel,
        explicit_outputs: explicit_outputs.iter().map(|name| path(name)).collect(),
        implicit_outputs: implicit_outputs.iter().map(|name| path(name)).collect(),
        order_only_deps: Vec::new(),
        progress_label: None,
        phony: false,
        always: false,
        output_visibility: crate::ir::EdgeOutputVisibility::default(),
    }
}