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
    Ok(edge_bindings(ninja, output)?
        .into_iter()
        .any(|line| line.trim() == "pool = console"))
}

/// Return every edge-local `pool` binding attached to the named build edge.
///
/// A console edge carries `pool = console`, an edge that inherits a console
/// action pool but does not opt in carries the empty `pool =` clearing
/// binding, and an edge with no console inheritance carries none. Returning
/// the bindings lets a test distinguish those three states instead of
/// reducing them to a Boolean.
///
/// # Examples
///
/// ```
/// use test_support::ninja_edge::edge_pool_bindings;
///
/// let ninja = "build ordinary: command\n  pool =\n";
/// assert_eq!(edge_pool_bindings(ninja, "ordinary")?, vec!["pool =".to_owned()]);
/// # Ok::<(), anyhow::Error>(())
/// ```
///
/// # Errors
///
/// Returns an error when the named build edge is absent.
pub fn edge_pool_bindings(ninja: &str, output: &str) -> Result<Vec<String>> {
    Ok(edge_bindings(ninja, output)?
        .into_iter()
        .filter(|line| line.trim_start().starts_with("pool ="))
        .map(|line| line.trim().to_owned())
        .collect())
}

/// Return the indented binding lines that follow the named build statement.
///
/// Ninja ends an edge at the first line that is not indented, so taking the
/// contiguous indented run scopes every lookup to one build statement and
/// keeps a neighbouring edge's binding from satisfying an assertion.
fn edge_bindings<'a>(ninja: &'a str, output: &str) -> Result<Vec<&'a str>> {
    let lines: Vec<_> = ninja.lines().collect();
    let edge_index = lines
        .iter()
        .position(|line| build_edge_has_output(line, output))
        .with_context(|| format!("missing Ninja build edge for {output}"))?;

    Ok(lines
        .iter()
        .skip(edge_index + 1)
        .take_while(|line| line.starts_with("  "))
        .copied()
        .collect())
}

/// Return whether a generated build statement names `output` in its output list.
fn build_edge_has_output(line: &str, output: &str) -> bool {
    let Some(outputs) = line
        .strip_prefix("build ")
        .and_then(|statement| statement.split_once(": ").map(|(outputs, _)| outputs))
    else {
        return false;
    };

    let ninja_output = output.replace(' ', "$ ");
    ninja_output_tokens(outputs)
        .iter()
        .any(|candidate| !candidate.is_empty() && candidate == &ninja_output)
}

/// Split generated Ninja outputs while retaining escaped spaces within a path.
fn ninja_output_tokens(outputs: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut token = String::new();
    let mut is_escaped = false;

    for character in outputs.chars() {
        match character {
            '$' if !is_escaped => {
                token.push('$');
                is_escaped = true;
            }
            escaped if is_escaped => {
                token.push(escaped);
                is_escaped = false;
            }
            delimiter if delimiter.is_whitespace() || delimiter == '|' => {
                tokens.push(std::mem::take(&mut token));
            }
            other => token.push(other),
        }
    }

    tokens.push(token);

    tokens
}

#[cfg(test)]
mod tests {
    //! Cover output-list matching for generated Ninja build statements.

    use super::{edge_has_console_pool, edge_pool_bindings};
    use anyhow::{Result, ensure};

    #[test]
    fn console_pool_lookup_matches_explicit_and_implicit_outputs() -> Result<()> {
        let ninja = "build first second | implicit: command\n  pool = console\n";

        ensure!(edge_has_console_pool(ninja, "second")?);
        ensure!(edge_has_console_pool(ninja, "implicit")?);
        Ok(())
    }

    #[test]
    fn console_pool_lookup_matches_escaped_spaces_in_outputs() -> Result<()> {
        let ninja = "build first$ output: command\n  pool = console\n";

        ensure!(edge_has_console_pool(ninja, "first output")?);
        Ok(())
    }

    #[test]
    fn console_pool_lookup_does_not_match_a_partial_output() {
        let ninja = "build target-extra: command\n  pool = console\n";

        assert!(edge_has_console_pool(ninja, "target").is_err());
    }

    #[test]
    fn pool_bindings_distinguish_selected_cleared_and_absent_states() -> Result<()> {
        let ninja = concat!(
            "build selected: command\n  pool = console\n\n",
            "build cleared: command\n  pool =\n\n",
            "build absent: command\n  command = touch absent\n",
        );

        ensure!(edge_pool_bindings(ninja, "selected")? == vec!["pool = console".to_owned()]);
        ensure!(edge_pool_bindings(ninja, "cleared")? == vec!["pool =".to_owned()]);
        ensure!(edge_pool_bindings(ninja, "absent")?.is_empty());
        Ok(())
    }

    #[test]
    fn pool_bindings_are_scoped_to_the_named_edge() -> Result<()> {
        let ninja = "build first: command\n  pool = console\n\nbuild second: command\n";

        ensure!(edge_pool_bindings(ninja, "first")? == vec!["pool = console".to_owned()]);
        ensure!(edge_pool_bindings(ninja, "second")?.is_empty());
        Ok(())
    }

    #[test]
    fn pool_bindings_report_a_missing_edge() {
        assert!(edge_pool_bindings("build present: command\n", "absent").is_err());
    }
}
