//! Step definitions for typed CLI configuration flags.

use crate::bdd::fixtures::{RefCellOptionExt, TestWorld};
use crate::bdd::helpers::assertions::normalize_fluent_isolates;
use anyhow::{Context, Result, ensure};
use netsuke::cli::config::{AccessibilityPolicy, ColourPolicy, EmojiPolicy, ProgressPolicy};
use rstest_bdd_macros::then;

fn normalised_csv(items: &[String]) -> String {
    items.join(", ")
}

fn assert_cli_field<T>(
    world: &TestWorld,
    field_name: &str,
    expected: T,
    extract: impl FnOnce(&netsuke::cli::Cli) -> T,
) -> Result<()>
where
    T: std::fmt::Display + PartialEq + Copy,
{
    let actual = world
        .cli
        .with_ref(extract)
        .context(format!("CLI {field_name} should be present"))?;
    ensure!(
        actual == expected,
        "expected {field_name} {expected}, got {actual}"
    );
    Ok(())
}

#[then("the color policy is {expected:string}")]
fn color_policy_is(world: &TestWorld, expected: ColourPolicy) -> Result<()> {
    assert_cli_field(world, "color policy", expected, |cli| cli.color)
}

#[then("the emoji policy is {expected:string}")]
fn emoji_policy_is(world: &TestWorld, expected: EmojiPolicy) -> Result<()> {
    assert_cli_field(world, "emoji policy", expected, |cli| cli.emoji)
}

#[then("the progress policy is {expected:string}")]
fn progress_policy_is(world: &TestWorld, expected: ProgressPolicy) -> Result<()> {
    assert_cli_field(world, "progress policy", expected, |cli| cli.progress)
}

#[then("the accessibility policy is {expected:string}")]
fn accessibility_policy_is(world: &TestWorld, expected: AccessibilityPolicy) -> Result<()> {
    assert_cli_field(world, "accessibility policy", expected, |cli| {
        cli.accessibility
    })
}

#[then("the default targets are {expected:string}")]
fn default_targets_are(world: &TestWorld, expected: String) -> Result<()> {
    let actual = world
        .cli
        .with_ref(|cli| Some(normalised_csv(&cli.default_targets)))
        .flatten()
        .context("CLI default targets should be present")?;
    ensure!(
        actual == expected,
        "expected default targets {expected}, got {actual}"
    );
    Ok(())
}

#[then("progress resolution is disabled")]
fn progress_resolution_is_disabled(world: &TestWorld) -> Result<()> {
    let resolved = world
        .cli
        .with_ref(netsuke::cli::Cli::progress_enabled)
        .context("CLI should be present")?;
    ensure!(!resolved, "expected resolved progress to be disabled");
    Ok(())
}

#[then("JSON output is enabled")]
fn json_output_is_enabled(world: &TestWorld) -> Result<()> {
    let resolved = world
        .cli
        .with_ref(|cli| cli.json)
        .context("CLI should be present")?;
    ensure!(resolved, "expected JSON output to be enabled");
    Ok(())
}

#[then("the localized error contains {expected:string}")]
fn localized_error_contains(world: &TestWorld, expected: String) -> Result<()> {
    let actual = world
        .cli_error
        .get()
        .context("CLI error should be present")?;
    let normalised_actual = normalize_fluent_isolates(&actual);
    let normalised_expected = normalize_fluent_isolates(&expected);
    ensure!(
        normalised_actual.contains(&normalised_expected),
        "expected localized error to contain '{expected}', got '{actual}'"
    );
    Ok(())
}
