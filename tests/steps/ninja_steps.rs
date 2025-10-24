//! Step definitions for Ninja file generation scenarios.
#![expect(
    clippy::shadow_reuse,
    reason = "Cucumber step macros rebind capture names"
)]

use super::common::build_graph_available;
use crate::CliWorld;
use anyhow::{Context, Result, anyhow, ensure};
use cucumber::{then, when};
use netsuke::ninja_gen;

/// Assert that optional Ninja output or error content contains an expected fragment.
fn assert_contains(
    content: Option<&String>,
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

#[when("the ninja file is generated")]
fn generate_ninja(world: &mut CliWorld) -> Result<()> {
    let graph = build_graph_available(world)?;
    match ninja_gen::generate(graph) {
        Ok(n) => {
            world.ninja = Some(n);
            world.ninja_error = None;
        }
        Err(e) => {
            world.ninja = None;
            world.ninja_error = Some(e.to_string());
        }
    }
    Ok(())
}

#[then(expr = "the ninja file contains {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
fn ninja_contains(world: &mut CliWorld, expected_fragment: String) -> Result<()> {
    assert_contains(world.ninja.as_ref(), &expected_fragment, "ninja content")
}

#[then(expr = "shlex splitting command {int} yields {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
fn ninja_command_tokens(
    world: &mut CliWorld,
    command_index: usize,
    expected_tokens: String,
) -> Result<()> {
    ensure!(command_index > 0, "command index must be >= 1");
    let ninja = world
        .ninja
        .as_ref()
        .context("ninja content should be available")?;
    let commands: Vec<&str> = ninja
        .lines()
        .filter(|l| l.trim_start().starts_with("command ="))
        .collect();
    let index = command_index - 1;
    let line = commands
        .get(index)
        .with_context(|| format!("command index {command_index} out of range"))?;
    let command = line.trim_start().trim_start_matches("command = ");
    let words =
        shlex::split(command).ok_or_else(|| anyhow!("failed to split command '{command}'"))?;
    let expected: Vec<String> = expected_tokens
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
}

#[then(expr = "shlex splitting the command yields {string}")]
fn ninja_first_command_tokens(world: &mut CliWorld, expected_tokens: String) -> Result<()> {
    ninja_command_tokens(world, 2, expected_tokens)
}

#[then(expr = "ninja generation fails with {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
fn ninja_generation_fails(world: &mut CliWorld, expected_fragment: String) -> Result<()> {
    assert_contains(
        world.ninja_error.as_ref(),
        &expected_fragment,
        "ninja error",
    )
}

#[then("ninja generation fails mentioning the removed action id")]
fn ninja_generation_fails_with_removed_action_id(world: &mut CliWorld) -> Result<()> {
    let err = world
        .ninja_error
        .as_ref()
        .context("ninja error should be available")?;
    let id = world
        .removed_action_id
        .as_ref()
        .context("removed action id should be available")?;
    ensure!(
        err.contains(id),
        "ninja error '{err}' does not mention removed action id '{id}'"
    );
    Ok(())
}
