//! Step definitions for Ninja file generation scenarios.

use crate::bdd::fixtures::{RefCellOptionExt, TestWorld};
use crate::bdd::helpers::assertions::assert_optional_contains;
use crate::bdd::types::{ContentName, NinjaFragment, TokenList};
use anyhow::{Context, Result, anyhow, ensure};
use netsuke::ninja_gen;
use rstest_bdd_macros::{then, when};

// ---------------------------------------------------------------------------
// Typed helper functions
// ---------------------------------------------------------------------------

/// Get the ninja content from the world, returning a Result.
fn get_ninja_content(world: &TestWorld) -> Result<String> {
    world
        .ninja_content
        .get()
        .context("ninja content should be available")
}

/// Extract command line at the given 1-based index from ninja content.
fn extract_command_line(ninja: &str, index: usize) -> Result<String> {
    ensure!(index > 0, "command index must be >= 1");
    let commands: Vec<&str> = ninja
        .lines()
        .filter(|l| l.trim_start().starts_with("command ="))
        .collect();
    let idx = index - 1;
    let line = commands
        .get(idx)
        .with_context(|| format!("command index {index} out of range"))?;
    Ok(line
        .trim_start()
        .trim_start_matches("command = ")
        .to_owned())
}

/// Parse command line into tokens using shlex.
fn parse_command_tokens(command: &str) -> Result<Vec<String>> {
    shlex::split(command).ok_or_else(|| anyhow!("failed to split command '{command}'"))
}

/// Compare parsed tokens against expected token list.
fn compare_tokens(actual: &[String], expected: &TokenList) -> Result<()> {
    let expected_vec = expected.to_vec();
    ensure!(
        actual == expected_vec,
        "expected tokens {:?}, got {:?}",
        expected_vec,
        actual
    );
    Ok(())
}

/// Assert that optional content contains an expected fragment.
fn assert_content_contains(
    content: Option<String>,
    fragment: &NinjaFragment,
    name: ContentName,
) -> Result<()> {
    assert_optional_contains(content, fragment.as_str(), name.as_str())
}

/// Assert that error message mentions the removed action id.
fn assert_error_mentions_action_id(world: &TestWorld) -> Result<()> {
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
}

// ---------------------------------------------------------------------------
// When steps
// ---------------------------------------------------------------------------

#[when("the ninja file is generated")]
fn generate_ninja(world: &TestWorld) -> Result<()> {
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
}

// ---------------------------------------------------------------------------
// Then steps
// ---------------------------------------------------------------------------

#[then("the ninja file contains {fragment:string}")]
fn ninja_contains(world: &TestWorld, fragment: &str) -> Result<()> {
    assert_content_contains(
        world.ninja_content.get(),
        &NinjaFragment::new(fragment),
        ContentName::NinjaContent,
    )
}

#[then("shlex splitting command {index:usize} yields {tokens:string}")]
fn ninja_command_tokens(world: &TestWorld, index: usize, tokens: &str) -> Result<()> {
    let ninja = get_ninja_content(world)?;
    let command = extract_command_line(&ninja, index)?;
    let actual = parse_command_tokens(&command)?;
    compare_tokens(&actual, &TokenList::new(tokens))
}

/// Verify tokenization of the first user-defined command in the Ninja output.
///
/// Uses index 2 because indices 0-1 are Ninja preamble (ninja_required_version
/// and builddir declarations). Index 2 is typically the first build rule command.
#[then("shlex splitting the command yields {tokens:string}")]
fn ninja_first_command_tokens(world: &TestWorld, tokens: &str) -> Result<()> {
    ninja_command_tokens(world, 2, tokens)
}

#[then("ninja generation fails with {fragment:string}")]
fn ninja_generation_fails(world: &TestWorld, fragment: &str) -> Result<()> {
    assert_content_contains(
        world.ninja_error.get(),
        &NinjaFragment::new(fragment),
        ContentName::NinjaError,
    )
}

#[then("ninja generation fails mentioning the removed action id")]
fn ninja_generation_fails_with_removed_action_id(world: &TestWorld) -> Result<()> {
    assert_error_mentions_action_id(world)
}
