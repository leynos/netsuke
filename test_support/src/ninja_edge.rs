//! Inspect named build-edge bindings in generated Ninja text.
//!
//! This module gives tests one shared way to check edge-local metadata such as
//! `pool = console`. Keep it limited to simple generated-manifest assertions;
//! parsing arbitrary Ninja syntax belongs in a real parser.

use anyhow::{Context, Result};

/// Return whether the named output edge selects Ninja's console pool.
///
/// The lookup is scoped to the selected `build` statement, so a pool binding
/// inherited from a rule or attached to a neighbouring edge cannot satisfy the
/// assertion.
///
/// # Examples
///
/// ```
/// use test_support::ninja_edge::edge_has_console_pool;
///
/// let ninja = "build lint: command\n  pool = console\n";
/// assert!(edge_has_console_pool(ninja, "lint")?);
/// # Ok::<(), anyhow::Error>(())
/// ```
///
/// # Errors
///
/// Returns an error when the named build edge is absent.
pub fn edge_has_console_pool(ninja: &str, output: &str) -> Result<bool> {
    let lines: Vec<_> = ninja.lines().collect();
    let prefix = format!("build {output}:");
    let edge_index = lines
        .iter()
        .position(|line| line.starts_with(&prefix))
        .with_context(|| format!("missing Ninja build edge for {output}"))?;

    Ok(lines
        .iter()
        .skip(edge_index + 1)
        .take_while(|line| line.starts_with("  "))
        .any(|line| *line == "  pool = console"))
}
