//! Share stable action-rule and graph-path rendering across Ninja generators.
//!
//! This module owns the generator utilities used by direct Ninja output and
//! dyndep bundles. It keeps action ordering and path rendering consistent
//! without making runtime lookup part of the pure Ninja-generation boundary.

use std::fmt::Write;

use camino::Utf8PathBuf;
use itertools::Itertools;

use crate::{
    ir::{BuildEdge, BuildGraph},
    recipe_shell::BashExecutable,
};

use super::order::ordered_actions;
use super::{NamedAction, NinjaGenError, RecipeShell, path_syntax, validation};

/// Write executable rules in stable action-ID order, omitting dependency-only `phony` nodes.
///
/// # Errors
///
/// Returns [`NinjaGenError`] when an action recipe or metadata field cannot be
/// rendered safely, or when writing a generated rule fails.
pub(crate) fn write_action_rules<W: Write>(
    graph: &BuildGraph,
    out: &mut W,
    shell: RecipeShell,
    bash_executable: Option<&BashExecutable>,
) -> Result<(), NinjaGenError> {
    for (zero_based_action_index, (id, action)) in ordered_actions(graph).into_iter().enumerate() {
        if action.recipe.is_dependency_only() {
            continue;
        }
        validation::validate_action_recipe(action, zero_based_action_index + 1, shell)?;
        validation::validate_action_metadata(action)?;
        let named_action = NamedAction { id, action, shell };
        if let Some(executable) = bash_executable {
            named_action.write_into_with_bash(out, Some(executable))?;
        } else {
            named_action.write_into(out)?;
        }
    }
    Ok(())
}

/// Convert a slice of paths into a space-separated string.
pub(crate) fn join(paths: &[Utf8PathBuf]) -> String {
    paths
        .iter()
        .map(|path| path_syntax::clone_validated_ninja_path(path.as_str()))
        .join(" ")
}

/// Generate a stable key for a list of paths.
pub(crate) fn path_key(paths: &[Utf8PathBuf]) -> String {
    let mut parts: Vec<String> = paths.iter().map(|path| path.as_str().to_owned()).collect();
    parts.sort_unstable();
    parts.join(&char::from(0).to_string())
}

/// Return whether the graph contains a serial edge that needs dyndep gates.
pub(crate) fn graph_requires_dyndep(graph: &BuildGraph) -> bool {
    graph.edges().any(edge_requires_gates)
}

/// Return whether one edge needs staged serial dependency gates.
pub(crate) fn edge_requires_gates(edge: &BuildEdge) -> bool {
    edge.dependency_order == crate::ir::DependencyOrder::Serial && edge.implicit_deps.len() > 1
}
