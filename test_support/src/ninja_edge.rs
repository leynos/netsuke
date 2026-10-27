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
    let edge_index = lines
        .iter()
        .position(|line| build_edge_has_output(line, output))
        .with_context(|| format!("missing Ninja build edge for {output}"))?;

    Ok(lines
        .iter()
        .skip(edge_index + 1)
        .take_while(|line| line.starts_with("  "))
        .any(|line| *line == "  pool = console"))
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

    use super::edge_has_console_pool;
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
}
