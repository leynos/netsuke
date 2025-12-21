//! Step definitions for Ninja file generation scenarios.

use crate::bdd::fixtures::{RefCellOptionExt, strip_quotes, with_world};
use anyhow::{Context, Result, anyhow, ensure};
use netsuke::ninja_gen;
use rstest_bdd_macros::{then, when};

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Assert that optional content contains an expected fragment.
fn assert_contains(
    content: Option<String>,
    expected_fragment: &str,
    content_name: &str,
) -> Result<()> {
    let text = content.context(format!("{content_name} should be available"))?;
    ensure!(
        text.contains(expected_fragment),
        "{content_name} should contain '{expected_fragment}'"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// When steps
// ---------------------------------------------------------------------------

#[when("the ninja file is generated")]
fn generate_ninja() -> Result<()> {
    with_world(|world| {
        let result = world
            .build_graph
            .with_ref(|graph| ninja_gen::generate(graph));
        match result.context("build graph should be available")? {
            Ok(n) => {
                world.ninja_content.set(n);
                world.ninja_error.clear();
            }
            Err(e) => {
                world.ninja_content.clear();
                world.ninja_error.set(e.to_string());
            }
        }
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// Then steps
// ---------------------------------------------------------------------------

#[then("the ninja file contains {fragment}")]
fn ninja_contains(fragment: String) -> Result<()> {
    let fragment = strip_quotes(&fragment);
    with_world(|world| assert_contains(world.ninja_content.get(), fragment, "ninja content"))
}

#[then("shlex splitting command {index:usize} yields {tokens}")]
fn ninja_command_tokens(index: usize, tokens: String) -> Result<()> {
    let tokens = strip_quotes(&tokens);
    ensure!(index > 0, "command index must be >= 1");
    with_world(|world| {
        let ninja = world
            .ninja_content
            .get()
            .context("ninja content should be available")?;
        let commands: Vec<&str> = ninja
            .lines()
            .filter(|l| l.trim_start().starts_with("command ="))
            .collect();
        let idx = index - 1;
        let line = commands
            .get(idx)
            .with_context(|| format!("command index {index} out of range"))?;
        let command = line.trim_start().trim_start_matches("command = ");
        let words =
            shlex::split(command).ok_or_else(|| anyhow!("failed to split command '{command}'"))?;
        let expected: Vec<String> = tokens
            .split(',')
            .map(|w| w.trim().replace("\\n", "\n"))
            .collect();
        ensure!(
            words == expected,
            "expected tokens {:?}, got {:?}",
            expected,
            words
        );
        Ok(())
    })
}

#[then("shlex splitting the command yields {tokens}")]
fn ninja_first_command_tokens(tokens: String) -> Result<()> {
    // strip_quotes is applied in ninja_command_tokens
    ninja_command_tokens(2, tokens)
}

#[then("ninja generation fails with {fragment}")]
fn ninja_generation_fails(fragment: String) -> Result<()> {
    let fragment = strip_quotes(&fragment);
    with_world(|world| assert_contains(world.ninja_error.get(), fragment, "ninja error"))
}

#[then("ninja generation fails mentioning the removed action id")]
fn ninja_generation_fails_with_removed_action_id() -> Result<()> {
    with_world(|world| {
        let err = world
            .ninja_error
            .get()
            .context("ninja error should be available")?;
        let id = world
            .removed_action_id
            .get()
            .context("removed action id should be available")?;
        ensure!(
            err.contains(&id),
            "ninja error '{err}' does not mention removed action id '{id}'"
        );
        Ok(())
    })
}
