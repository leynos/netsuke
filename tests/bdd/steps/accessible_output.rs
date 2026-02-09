//! Step definitions for accessible output mode scenarios.
//!
//! Provides BDD step functions for output mode detection, verifying that
//! `OutputMode::resolve_with` correctly interprets environment signals and
//! explicit configuration. Steps use simulated environment variables stored
//! in [`TestWorld`] rather than mutating the real process environment.

use crate::bdd::fixtures::{RefCellOptionExt, TestWorld};
use crate::bdd::types::EnvVarValue;
use anyhow::{Result, ensure};
use netsuke::output_mode::{self, OutputMode};
use rstest_bdd_macros::{given, then, when};

// ---------------------------------------------------------------------------
// Helper: build an env lookup closure from TestWorld simulated values
// ---------------------------------------------------------------------------

/// Build an environment variable lookup closure from simulated values.
fn simulated_env(world: &TestWorld) -> impl Fn(&str) -> Option<String> + '_ {
    move |key| match key {
        "NO_COLOR" => world.simulated_no_color.get(),
        "TERM" => world.simulated_term.get(),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Given steps: configure simulated environment
// ---------------------------------------------------------------------------

#[expect(
    clippy::unnecessary_wraps,
    reason = "rstest-bdd macro generates Result wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[given("the simulated TERM is {value:string}")]
fn set_simulated_term(world: &TestWorld, value: EnvVarValue) -> Result<()> {
    world.simulated_term.set(value.into_string());
    Ok(())
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "rstest-bdd macro generates Result wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[given("the simulated NO_COLOR is {value:string}")]
fn set_simulated_no_color(world: &TestWorld, value: EnvVarValue) -> Result<()> {
    world.simulated_no_color.set(value.into_string());
    Ok(())
}

// ---------------------------------------------------------------------------
// When steps: resolve output mode
// ---------------------------------------------------------------------------

#[expect(
    clippy::unnecessary_wraps,
    reason = "rstest-bdd macro generates Result wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[when("the output mode is resolved with no explicit setting")]
fn resolve_no_explicit(world: &TestWorld) -> Result<()> {
    let mode = output_mode::resolve_with(None, simulated_env(world));
    world.output_mode.set(format!("{mode:?}"));
    Ok(())
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "rstest-bdd macro generates Result wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[when("the output mode is resolved with accessible set to true")]
fn resolve_accessible_true(world: &TestWorld) -> Result<()> {
    let mode = output_mode::resolve_with(Some(true), simulated_env(world));
    world.output_mode.set(format!("{mode:?}"));
    Ok(())
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "rstest-bdd macro generates Result wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[when("the output mode is resolved with accessible set to false")]
fn resolve_accessible_false(world: &TestWorld) -> Result<()> {
    let mode = output_mode::resolve_with(Some(false), simulated_env(world));
    world.output_mode.set(format!("{mode:?}"));
    Ok(())
}

// ---------------------------------------------------------------------------
// Then steps: verify output mode
// ---------------------------------------------------------------------------

fn verify_output_mode(world: &TestWorld, expected: OutputMode) -> Result<()> {
    let actual = world
        .output_mode
        .get()
        .ok_or_else(|| anyhow::anyhow!("output mode has not been resolved"))?;
    let expected_str = format!("{expected:?}");
    ensure!(
        actual == expected_str,
        "expected output mode {expected_str}, got {actual}"
    );
    Ok(())
}

#[then("the output mode is accessible")]
fn output_mode_is_accessible(world: &TestWorld) -> Result<()> {
    verify_output_mode(world, OutputMode::Accessible)
}

#[then("the output mode is standard")]
fn output_mode_is_standard(world: &TestWorld) -> Result<()> {
    verify_output_mode(world, OutputMode::Standard)
}

// ---------------------------------------------------------------------------
// Then steps: verify CLI accessible field
// ---------------------------------------------------------------------------

#[then]
fn accessible_mode_is_enabled(world: &TestWorld) -> Result<()> {
    let accessible = world
        .cli
        .with_ref(|cli| cli.accessible)
        .ok_or_else(|| anyhow::anyhow!("CLI has not been parsed"))?;
    ensure!(
        accessible == Some(true),
        "expected accessible to be Some(true), got {accessible:?}"
    );
    Ok(())
}

#[then]
fn accessible_mode_is_disabled(world: &TestWorld) -> Result<()> {
    let accessible = world
        .cli
        .with_ref(|cli| cli.accessible)
        .ok_or_else(|| anyhow::anyhow!("CLI has not been parsed"))?;
    ensure!(
        accessible == Some(false),
        "expected accessible to be Some(false), got {accessible:?}"
    );
    Ok(())
}
