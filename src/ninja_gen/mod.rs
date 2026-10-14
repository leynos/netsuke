//! Ninja file generator.
//!
//! This module converts a [`crate::ir::BuildGraph`] into the textual
//! representation expected by the Ninja build system. The generator sorts
//! actions and edges to ensure deterministic output for snapshot tests. The
//! generated Ninja file is written by the runner and `generate` command for
//! downstream execution by the Ninja build system.

pub mod dyndep;
mod path_syntax;
mod rule_utils;

use dyndep::reject_reserved_paths;
pub use dyndep::{GeneratedDyndep, GeneratedNinja, generate_bundle};
pub(crate) use path_syntax::{reject_unsupported_path_characters, validated_ninja_path};
pub(crate) use rule_utils::{
    edge_requires_gates, graph_requires_dyndep, join, path_key, write_action_rules,
};

use crate::ir::BuildGraph;
use crate::localization::{self, keys};
use std::fmt::Write;

pub(crate) mod command_list;
mod error;
mod explicit_shell;

mod escape;
mod recipe_shell;
mod validation;

pub use crate::recipe_shell::RecipeShell;
pub use error::NinjaGenError;
pub use explicit_shell::generate_with_shell;
/// Write `key = value` to a Ninja file when `opt` holds a value.
///
/// The indented assignment is emitted only for present values, so optional
/// fields vanish from the generated file instead of being left blank.
macro_rules! write_kv {
    ($f:expr, $key:expr, $opt:expr) => {
        if let Some(val) = $opt {
            writeln!($f, "  {} = {}", $key, val)?;
        }
    };
}
/// Write `key = 1` to a Ninja file when `cond` is set.
///
/// Boolean flags are rendered as a `1` only when enabled, matching Ninja's
/// convention for switch-like variables.
macro_rules! write_flag {
    ($f:expr, $key:expr, $cond:expr) => {
        if $cond {
            writeln!($f, "  {} = 1", $key)?;
        }
    };
}

mod display_edge;
mod named_action;
mod order;
pub(crate) use display_edge::DisplayEdge;
pub(crate) use named_action::NamedAction;
use order::{ordered_default_targets, ordered_edges};
/// Generate a Ninja build file as a string.
///
/// # Examples
/// ```
/// use netsuke::ast::{CommandIsolation, Recipe};
/// use netsuke::ir::{Action, BuildEdge, BuildGraph};
/// use camino::Utf8PathBuf;
/// let mut graph = BuildGraph::default();
/// graph.actions.insert("a".into(), Action {
///     recipe: Recipe::Command {
///         command: "true".into(),
///         isolation: CommandIsolation::Shared,
///     },
///     description: None, depfile: None, deps_format: None,
///     pool: None, restat: false
/// });
/// graph.insert_edge(BuildEdge {
///     action_id: "a".into(), inputs: Vec::new(),
///     implicit_deps: Vec::new(),
///     dependency_order: netsuke::ir::DependencyOrder::Parallel,
///     explicit_outputs: vec![Utf8PathBuf::from("out")],
///     implicit_outputs: Vec::new(), order_only_deps: Vec::new(),
///     phony: false, always: false
/// }).expect("test graph output aliases must be unique");
/// # let result: Result<(), netsuke::ninja_gen::NinjaGenError> = (|| {
/// let text = netsuke::ninja_gen::generate(&graph)?;
/// assert!(text.contains("rule a"));
/// # Ok(())
/// # })();
/// # assert!(result.is_ok());
/// ```
///
/// # Errors
///
/// Returns [`NinjaGenError`] if a build edge references an unknown action, a
/// programmatic action has an empty command recipe, a command-list entry starts
/// multiple background jobs, a command-list entry uses an unsupported `exec`
/// structure, a command-list `eval` payload cannot be analysed, a command-list
/// entry contains a Ninja control character, isolated mode is attached to a
/// non-list command, a PowerShell recipe exceeds its size limit, an isolated
/// PowerShell entry exceeds Windows' command-line limit, a graph path uses
/// Netsuke's reserved serial-ordering namespace, or writing to the output fails.
pub fn generate(graph: &BuildGraph) -> Result<String, NinjaGenError> {
    let mut out = String::new();
    generate_into(graph, &mut out)?;
    Ok(out)
}

/// Write a Ninja build file to the provided writer.
///
/// # Examples
/// ```
/// use netsuke::ast::{CommandIsolation, Recipe};
/// use netsuke::ir::{Action, BuildEdge, BuildGraph};
/// use camino::Utf8PathBuf;
/// let mut graph = BuildGraph::default();
/// graph.actions.insert("a".into(), Action {
///     recipe: Recipe::Command {
///         command: "true".into(),
///         isolation: CommandIsolation::Shared,
///     },
///     description: None, depfile: None, deps_format: None,
///     pool: None, restat: false
/// });
/// graph.insert_edge(BuildEdge {
///     action_id: "a".into(), inputs: Vec::new(),
///     implicit_deps: Vec::new(),
///     dependency_order: netsuke::ir::DependencyOrder::Parallel,
///     explicit_outputs: vec![Utf8PathBuf::from("out")],
///     implicit_outputs: Vec::new(), order_only_deps: Vec::new(),
///     phony: false, always: false
/// }).expect("test graph output aliases must be unique");
/// let mut out = String::new();
/// # let result: Result<(), netsuke::ninja_gen::NinjaGenError> = (|| {
/// netsuke::ninja_gen::generate_into(&graph, &mut out)?;
/// assert!(out.contains("build out: a"));
/// # Ok(())
/// # })();
/// # assert!(result.is_ok());
/// ```
///
/// # Errors
///
/// Returns [`NinjaGenError`] if a build edge references an unknown action, a
/// programmatic action has an empty command recipe, a command-list entry starts
/// multiple background jobs, a command-list entry uses an unsupported `exec`
/// structure, a command-list `eval` payload cannot be analysed, a command-list
/// entry contains a Ninja control character, isolated mode is attached to a
/// non-list command, a PowerShell recipe exceeds its size limit, an isolated
/// PowerShell entry exceeds Windows' command-line limit, a graph path uses
/// Netsuke's reserved serial-ordering namespace, or writing to the output fails.
pub fn generate_into<W: Write>(graph: &BuildGraph, out: &mut W) -> Result<(), NinjaGenError> {
    generate_into_with_shell(graph, out, RecipeShell::host_default())
}

/// Write a Ninja build file for one explicit legacy recipe interpreter.
///
/// # Errors
///
/// Returns [`NinjaGenError`] when the graph cannot be represented safely or
/// writing the generated output fails.
pub(crate) fn generate_into_with_shell<W: Write>(
    graph: &BuildGraph,
    out: &mut W,
    shell: RecipeShell,
) -> Result<(), NinjaGenError> {
    reject_unsupported_path_characters(graph)?;
    reject_reserved_paths(graph)?;
    if graph_requires_dyndep(graph) {
        return Err(NinjaGenError::DyndepFilesRequired {
            message: localization::message(keys::NINJA_GEN_DYNDEP_FILES_REQUIRED),
        });
    }
    write_action_rules(graph, out, shell, None)?;
    for edge in ordered_edges(graph) {
        let action =
            graph
                .actions
                .get(&edge.action_id)
                .ok_or_else(|| NinjaGenError::MissingAction {
                    id: edge.action_id.clone(),
                    message: localization::message(keys::NINJA_GEN_MISSING_ACTION)
                        .with_arg("id", &edge.action_id),
                })?;
        write!(
            out,
            "{}",
            DisplayEdge {
                edge,
                action_name: if action.recipe.is_dependency_only() {
                    "phony"
                } else {
                    &edge.action_id
                },
                action_restat: action.restat,
                implicit_deps: &edge.implicit_deps,
            }
        )?;
    }

    let defaults = ordered_default_targets(graph);
    if !defaults.is_empty() {
        writeln!(out, "default {}", join(&defaults))?;
    }

    Ok(())
}

#[cfg(test)]
mod determinism;
#[cfg(test)]
mod property_tests;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
