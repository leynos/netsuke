//! Step definitions for typed CLI configuration flags.

use crate::bdd::fixtures::{RefCellOptionExt, TestWorld};
use crate::bdd::helpers::assertions::normalize_fluent_isolates;
use anyhow::{Context, Result, ensure};
use rstest_bdd_macros::then;

fn normalised_csv(items: &[String]) -> String {
    items.join(", ")
}

#[then("the colour policy is {expected:string}")]
fn colour_policy_is(world: &TestWorld, expected: String) -> Result<()> {
    let actual = world
        .cli
        .with_ref(|cli| cli.colour_policy.map(|value| value.to_string()))
        .flatten()
        .context("CLI colour policy should be present")?;
    ensure!(
        actual == expected,
        "expected colour policy {expected}, got {actual}"
    );
    Ok(())
}

#[then("the spinner mode is {expected:string}")]
fn spinner_mode_is(world: &TestWorld, expected: String) -> Result<()> {
    let actual = world
        .cli
        .with_ref(|cli| cli.spinner_mode.map(|value| value.to_string()))
        .flatten()
        .context("CLI spinner mode should be present")?;
    ensure!(
        actual == expected,
        "expected spinner mode {expected}, got {actual}"
    );
    Ok(())
}

#[then("the output format is {expected:string}")]
fn output_format_is(world: &TestWorld, expected: String) -> Result<()> {
    let actual = world
        .cli
        .with_ref(|cli| cli.output_format.map(|value| value.to_string()))
        .flatten()
        .context("CLI output format should be present")?;
    ensure!(
        actual == expected,
        "expected output format {expected}, got {actual}"
    );
    Ok(())
}

#[then("the default targets are {expected:string}")]
fn default_targets_are(world: &TestWorld, expected: String) -> Result<()> {
    let actual = world
        .cli
        .with_ref(|cli| normalised_csv(&cli.default_targets))
        .context("CLI should be present")?;
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
        .with_ref(netsuke::cli::Cli::resolved_progress)
        .context("CLI should be present")?;
    ensure!(!resolved, "expected resolved progress to be disabled");
    Ok(())
}

#[then("diagnostic JSON resolution is enabled")]
fn diagnostic_json_resolution_is_enabled(world: &TestWorld) -> Result<()> {
    let resolved = world
        .cli
        .with_ref(netsuke::cli::Cli::resolved_diag_json)
        .context("CLI should be present")?;
    ensure!(resolved, "expected resolved diagnostic JSON to be enabled");
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
