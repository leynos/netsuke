//! Step definitions for accessibility preference scenarios.
//!
//! Provides BDD step functions for output preference resolution, verifying
//! that `OutputPrefs::resolve_with` correctly interprets environment signals
//! and explicit configuration. Steps use simulated environment variables
//! stored in [`TestWorld`] rather than mutating the real process environment.

use crate::bdd::fixtures::TestWorld;
use crate::bdd::types::EnvVarValue;
use anyhow::{Result, ensure};
use netsuke::output_prefs;
use rstest_bdd_macros::{given, then, when};

// ---------------------------------------------------------------------------
// Helper: build an env lookup closure from TestWorld simulated values
// ---------------------------------------------------------------------------

/// Build an environment variable lookup closure from simulated values.
fn simulated_env(world: &TestWorld) -> impl Fn(&str) -> Option<String> + '_ {
    move |key| match key {
        "NO_COLOR" => world.simulated_no_color.get(),
        "NETSUKE_NO_EMOJI" => world.simulated_no_emoji.get(),
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
#[given("the simulated NETSUKE_NO_EMOJI is {value:string}")]
fn set_simulated_netsuke_no_emoji(world: &TestWorld, value: EnvVarValue) -> Result<()> {
    world.simulated_no_emoji.set(value.into_string());
    Ok(())
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "rstest-bdd macro generates Result wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[given("emoji is suppressed")]
fn emoji_is_suppressed(world: &TestWorld) -> Result<()> {
    let prefs = output_prefs::resolve_with(Some(true), |_| None);
    world.output_prefs.set(prefs);
    Ok(())
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "rstest-bdd macro generates Result wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[given("emoji is allowed")]
fn emoji_is_allowed(world: &TestWorld) -> Result<()> {
    let prefs = output_prefs::resolve_with(None, |_| None);
    world.output_prefs.set(prefs);
    Ok(())
}

// ---------------------------------------------------------------------------
// When steps: resolve output preferences
// ---------------------------------------------------------------------------

#[expect(
    clippy::unnecessary_wraps,
    reason = "rstest-bdd macro generates Result wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[when("output preferences are resolved with no explicit setting")]
fn resolve_no_explicit(world: &TestWorld) -> Result<()> {
    let prefs = output_prefs::resolve_with(None, simulated_env(world));
    world.output_prefs.set(prefs);
    Ok(())
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "rstest-bdd macro generates Result wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[when("output preferences are resolved with no_emoji set to true")]
fn resolve_no_emoji_true(world: &TestWorld) -> Result<()> {
    let prefs = output_prefs::resolve_with(Some(true), simulated_env(world));
    world.output_prefs.set(prefs);
    Ok(())
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "rstest-bdd macro generates Result wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[when("output preferences are resolved with no_emoji set to false")]
fn resolve_no_emoji_false(world: &TestWorld) -> Result<()> {
    let prefs = output_prefs::resolve_with(Some(false), simulated_env(world));
    world.output_prefs.set(prefs);
    Ok(())
}

/// Retrieve resolved output preferences and render a prefix via `prefix_fn`.
fn render_prefix_with(
    world: &TestWorld,
    prefix_fn: impl FnOnce(&output_prefs::OutputPrefs) -> String,
) -> Result<()> {
    let prefs = world
        .output_prefs
        .get()
        .ok_or_else(|| anyhow::anyhow!("output prefs have not been resolved"))?;
    world.rendered_prefix.set(prefix_fn(&prefs));
    Ok(())
}

#[when("the error prefix is rendered")]
fn render_error_prefix(world: &TestWorld) -> Result<()> {
    render_prefix_with(world, |prefs| prefs.error_prefix().to_string())
}

#[when("the warning prefix is rendered")]
fn render_warning_prefix(world: &TestWorld) -> Result<()> {
    render_prefix_with(world, |prefs| prefs.warning_prefix().to_string())
}

#[when("the success prefix is rendered")]
fn render_success_prefix(world: &TestWorld) -> Result<()> {
    render_prefix_with(world, |prefs| prefs.success_prefix().to_string())
}

// ---------------------------------------------------------------------------
// Then steps: verify output preferences
// ---------------------------------------------------------------------------

fn verify_emoji(world: &TestWorld, expected: bool) -> Result<()> {
    let prefs = world
        .output_prefs
        .get()
        .ok_or_else(|| anyhow::anyhow!("output prefs have not been resolved"))?;
    ensure!(
        prefs.emoji_allowed() == expected,
        "expected emoji_allowed() to be {expected}, got {}",
        prefs.emoji_allowed()
    );
    Ok(())
}

#[then("emoji is disabled")]
fn emoji_is_disabled_then(world: &TestWorld) -> Result<()> {
    verify_emoji(world, false)
}

#[then("emoji is enabled")]
fn emoji_is_enabled_then(world: &TestWorld) -> Result<()> {
    verify_emoji(world, true)
}

#[then("the prefix contains {expected:string}")]
fn prefix_contains(world: &TestWorld, expected: EnvVarValue) -> Result<()> {
    let rendered = world
        .rendered_prefix
        .get()
        .ok_or_else(|| anyhow::anyhow!("prefix has not been rendered"))?;
    let expected_str = expected.as_str();
    ensure!(
        rendered.contains(expected_str),
        "expected prefix to contain '{expected_str}', got '{rendered}'"
    );
    Ok(())
}

#[then("the prefix contains no non-ASCII characters")]
fn prefix_is_ascii(world: &TestWorld) -> Result<()> {
    let rendered = world
        .rendered_prefix
        .get()
        .ok_or_else(|| anyhow::anyhow!("prefix has not been rendered"))?;
    ensure!(
        rendered.is_ascii(),
        "expected ASCII-only prefix, got '{rendered}'"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Then steps: verify CLI no_emoji field
// ---------------------------------------------------------------------------

fn verify_no_emoji_mode(world: &TestWorld, expected: Option<bool>) -> Result<()> {
    use crate::bdd::fixtures::RefCellOptionExt;
    let no_emoji = world
        .cli
        .with_ref(|cli| cli.no_emoji)
        .ok_or_else(|| anyhow::anyhow!("CLI has not been parsed"))?;
    ensure!(
        no_emoji == expected,
        "expected no_emoji to be {expected:?}, got {no_emoji:?}"
    );
    Ok(())
}

#[then]
fn no_emoji_mode_is_enabled(world: &TestWorld) -> Result<()> {
    verify_no_emoji_mode(world, Some(true))
}

#[then]
fn no_emoji_mode_is_disabled(world: &TestWorld) -> Result<()> {
    verify_no_emoji_mode(world, Some(false))
}
