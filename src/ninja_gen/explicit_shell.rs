//! Generates Ninja text with a caller-selected legacy recipe interpreter.

use crate::ir::BuildGraph;

use super::{NinjaGenError, RecipeShell, generate_into_with_shell};

/// Generate a Ninja build file for one explicit legacy recipe interpreter.
///
/// The Netsuke CLI resolves the interpreter from its host contract before
/// generation. Library consumers can select an explicit renderer when they
/// need deterministic cross-platform Ninja text.
///
/// # Errors
///
/// Returns [`NinjaGenError`] when the graph cannot be represented safely for
/// the selected interpreter or writing the output fails.
pub fn generate_with_shell(
    graph: &BuildGraph,
    shell: RecipeShell,
) -> Result<String, NinjaGenError> {
    let mut out = String::new();
    generate_into_with_shell(graph, &mut out, shell)?;
    Ok(out)
}
