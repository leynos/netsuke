//! Staged Ninja dyndep lowering for serial dependency ordering.
//!
//! Netsuke keeps one top-level Ninja invocation. To make a `serial` `deps`
//! list execute in declaration order, this module lowers each serial edge
//! into a chain of phony gates plus content-addressed dyndep sidecars. Each
//! gate names one sidecar through Ninja `dyndep` binding; the sidecar reveals
//! exactly one *relay* phony, and the relay carries the real dependency. The
//! next sidecar is not visible until the preceding gate completes, so Ninja
//! cannot schedule a later dependency before an earlier one succeeds.
//!
//! The relay exists to keep Ninja out of a crash in its own scheduler. Loading
//! a sidecar appends an edge to the revealed node's out-edge list, and Ninja
//! does that while iterating that same node's list when the revealed node is
//! already scheduled — reachable when a user edge is an order-only dependency
//! of a node a later sidecar reveals (`ninja-build/ninja#2662`). Revealing a
//! relay means the appended edge belongs to a node created for the purpose,
//! which no user edge names, so no iteration is ever in progress over it.
//!
//! The sidecars are immutable and content-addressed beneath `.netsuke/dyndep`,
//! and the gates live beneath `.netsuke/serial`. The runner materializes the
//! sidecars (and the main file) before invoking Ninja; string-only generation
//! rejects graphs that require a bundle.
//!
//! ```rust
//! use netsuke::ast::{CommandIsolation, Recipe};
//! use netsuke::ir::{BuildEdge, BuildGraph, DependencyOrder};
//! use netsuke::ninja_gen::generate_bundle;
//! use camino::Utf8PathBuf;
//!
//! let action = netsuke::ir::Action {
//!     recipe: Recipe::Command {
//!         command: "echo done".into(),
//!         isolation: CommandIsolation::Shared,
//!     },
//!     description: None,
//!     depfile: None,
//!     deps_format: None,
//!     pool: None,
//!     restat: false,
//! };
//! let mut graph = BuildGraph::default();
//! graph.actions.insert("a".into(), action);
//! graph.insert_edge(BuildEdge {
//!         action_id: "a".into(),
//!         inputs: Vec::new(),
//!         implicit_deps: vec![
//!             Utf8PathBuf::from("check-fmt"),
//!             Utf8PathBuf::from("test"),
//!         ],
//!         dependency_order: DependencyOrder::Serial,
//!         explicit_outputs: vec![Utf8PathBuf::from("all")],
//!         implicit_outputs: Vec::new(),
//!         order_only_deps: Vec::new(),
//!         phony: false,
//!         always: false,
//! }).expect("test graph output aliases must be unique");
//! let bundle = generate_bundle(&graph).expect("generate bundle");
//! assert!(bundle.build_file().contains("ninja_required_version = 1.10"));
//! assert_eq!(bundle.dyndep_files().len(), 2);
//! ```

mod bundle;
pub use bundle::{GeneratedDyndep, GeneratedNinja};

mod staging;
use staging::{SerialStages, render_serial_block};

use crate::ir::{BuildEdge, BuildGraph};
use crate::localization::{self, keys};
use crate::ninja_gen::{
    NinjaGenError, RecipeShell, edge_requires_gates, graph_requires_dyndep, join,
    ordered_default_targets, ordered_edges, reject_unsupported_path_characters, write_action_rules,
};
use crate::recipe_shell::BashExecutable;
use camino::Utf8PathBuf;
use std::fmt::Write as _;

/// Reserved state namespace for serial gate paths.
const SERIAL_NAMESPACE: &str = ".netsuke/serial";
/// Reserved state namespace for dyndep sidecar files.
const DYNDEP_NAMESPACE: &str = ".netsuke/dyndep";

/// Generate a complete Ninja bundle for `graph`, materializing staged dyndep
/// sidecars for every multi-dependency serial edge.
///
/// Ordinary parallel graphs produce a bundle with an empty sidecar list and a
/// main file identical to [`crate::ninja_gen::generate`].
///
/// # Errors
///
/// Returns [`NinjaGenError::ReservedOutputPath`] when a user output or
/// dependency collides with the reserved `.netsuke/serial` or
/// `.netsuke/dyndep` namespace, and [`NinjaGenError::MissingAction`] when an
/// edge references an unknown action.
pub fn generate_bundle(graph: &BuildGraph) -> Result<GeneratedNinja, NinjaGenError> {
    generate_bundle_for_shell(graph, RecipeShell::host_default())
}

/// Generate a complete Ninja bundle for one explicit legacy recipe interpreter.
///
/// # Errors
///
/// Returns [`NinjaGenError`] when the graph cannot be represented safely or
/// the generated bundle cannot be written.
pub(crate) fn generate_bundle_for_shell(
    graph: &BuildGraph,
    shell: RecipeShell,
) -> Result<GeneratedNinja, NinjaGenError> {
    generate_bundle_for_shell_with_bash(graph, shell, None)
}

/// Generate one bundle with an optional runner-selected Bash executable.
///
/// # Errors
///
/// Returns [`NinjaGenError`] when the graph cannot be represented safely or
/// the generated bundle cannot be written.
pub(crate) fn generate_bundle_for_shell_with_bash(
    graph: &BuildGraph,
    shell: RecipeShell,
    bash_executable: Option<&BashExecutable>,
) -> Result<GeneratedNinja, NinjaGenError> {
    generate_bundle_inner(graph, shell, bash_executable)
}

/// Construct the [`GeneratedNinja`] bundle for `graph`.
///
/// # Errors
///
/// Returns [`NinjaGenError::UnsupportedPathCharacter`] when a graph path
/// contains a character Ninja cannot represent, and
/// [`NinjaGenError::ReservedOutputPath`] when a graph path collides with the
/// reserved serial-ordering namespaces. Returns
/// [`NinjaGenError::EmptyCommandRecipe`],
/// [`NinjaGenError::MultipleBackgroundJobs`],
/// [`NinjaGenError::UnsupportedCommandListExec`],
/// [`NinjaGenError::UnanalyzableCommandListEval`], or
/// [`NinjaGenError::NinjaControlCharacter`] when an action recipe cannot be
/// rendered safely. Returns [`NinjaGenError::MissingAction`] when an edge
/// references an action absent from the graph, or [`NinjaGenError::Format`]
/// when rendering the bundle fails.
fn generate_bundle_inner(
    graph: &BuildGraph,
    shell: RecipeShell,
    bash_executable: Option<&BashExecutable>,
) -> Result<GeneratedNinja, NinjaGenError> {
    reject_unsupported_path_characters(graph)?;
    reject_reserved_paths(graph)?;
    let serial_present = graph_requires_dyndep(graph);

    let mut out = String::new();
    if serial_present {
        writeln!(out, "ninja_required_version = 1.10\n")?;
    }

    write_action_rules(graph, &mut out, shell, bash_executable)?;

    let mut stages = SerialStages::default();
    render_edges(graph, &mut out, &mut stages)?;

    let defaults = ordered_default_targets(graph);
    if !defaults.is_empty() {
        writeln!(out, "default {}", join(&defaults))?;
    }

    Ok(GeneratedNinja {
        build_file: out,
        dyndep_files: stages.dyndep_files,
    })
}

/// Render each distinct graph edge in stable output order.
fn render_edges(
    graph: &BuildGraph,
    out: &mut String,
    stages: &mut SerialStages,
) -> Result<(), NinjaGenError> {
    for edge in ordered_edges(graph.edges()) {
        render_edge(graph, edge, out, stages)?;
    }
    Ok(())
}

/// Render one graph edge, applying staged dyndep lowering when required.
fn render_edge(
    graph: &BuildGraph,
    edge: &BuildEdge,
    out: &mut String,
    stages: &mut SerialStages,
) -> Result<(), NinjaGenError> {
    let action =
        graph
            .actions
            .get(&edge.action_id)
            .ok_or_else(|| NinjaGenError::MissingAction {
                id: edge.action_id.clone(),
                message: localization::message(keys::NINJA_GEN_MISSING_ACTION)
                    .with_arg("id", &edge.action_id),
            })?;

    let rendered_action = RenderedAction {
        name: if action.recipe.is_dependency_only() {
            "phony"
        } else {
            &edge.action_id
        },
        restat: action.restat,
    };
    if edge_requires_gates(edge) {
        return render_serial_edge(edge, rendered_action, out, stages);
    }
    render_display_edge(edge, rendered_action, &edge.implicit_deps, out)
}

/// Hold the selected Ninja rule metadata while lowering a graph edge.
///
/// Keep these fields together because direct and serial lowering must render
/// the same rule selection without widening the graph representation.
#[derive(Clone, Copy)]
struct RenderedAction<'a> {
    /// Name the Ninja rule selected for the edge.
    name: &'a str,
    /// Preserve whether Ninja should restat this edge after execution.
    restat: bool,
}
/// Render one staged serial edge and replace its real dependencies with gates.
fn render_serial_edge(
    edge: &BuildEdge,
    action: RenderedAction<'_>,
    out: &mut String,
    stages: &mut SerialStages,
) -> Result<(), NinjaGenError> {
    let mut gate_paths = Vec::new();
    render_serial_block(edge, out, stages, &mut gate_paths)?;
    render_display_edge(edge, action, &gate_paths, out)
}

/// Render an edge using its already selected Ninja action metadata.
fn render_display_edge(
    edge: &BuildEdge,
    action: RenderedAction<'_>,
    implicit_deps: &[Utf8PathBuf],
    out: &mut String,
) -> Result<(), NinjaGenError> {
    write!(
        out,
        "{}",
        crate::ninja_gen::DisplayEdge {
            edge,
            action_name: action.name,
            action_restat: action.restat,
            implicit_deps,
        }
    )?;
    Ok(())
}

/// Reject user outputs or dependencies that collide with the reserved
/// serial-ordering state namespace.
pub(crate) fn reject_reserved_paths(graph: &BuildGraph) -> Result<(), NinjaGenError> {
    for edge in graph.edges() {
        for path in edge
            .explicit_outputs
            .iter()
            .chain(&edge.implicit_outputs)
            .chain(&edge.inputs)
            .chain(&edge.implicit_deps)
            .chain(&edge.order_only_deps)
        {
            let as_str = path.as_str();
            let is_reserved = [SERIAL_NAMESPACE, DYNDEP_NAMESPACE]
                .iter()
                .any(|namespace| {
                    as_str == *namespace
                        || as_str
                            .strip_prefix(namespace)
                            .is_some_and(|suffix| suffix.starts_with('/'))
                });
            if is_reserved {
                return Err(NinjaGenError::ReservedOutputPath {
                    path: path.clone(),
                    message: localization::message(keys::NINJA_GEN_RESERVED_OUTPUT_PATH)
                        .with_arg("path", as_str),
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
