//! Contracts for the build footprint rule's settings.
//!
//! The Concordat `rust-dev-build-footprint` rule needs the dev profile to
//! disable incremental compilation and debug information, and the pinned
//! nightly to take Cargo's native `embed-metadata = false` rather than a raw
//! rustc flag. These tests keep an edit to `Cargo.toml` or
//! `.cargo/config.toml` from quietly undoing any of the three.
#![cfg(all(unix, target_os = "linux"))]

use anyhow::{Result, ensure};
use test_support::build_tools::cargo_config;

/// The build footprint rule's three settings: the dev profile disables
/// incremental compilation and debug information, and the pinned nightly takes
/// Cargo's native `embed-metadata = false` rather than a raw rustc flag.
#[test]
fn the_build_footprint_settings_are_committed() -> Result<()> {
    let manifest: toml::Value = toml::from_str(include_str!("../Cargo.toml"))?;
    let dev = manifest
        .get("profile")
        .and_then(|profile| profile.get("dev"));
    ensure!(
        dev.and_then(|table| table.get("incremental"))
            .and_then(toml::Value::as_bool)
            == Some(false),
        "[profile.dev] must set incremental = false"
    );
    let debug = dev.and_then(|table| table.get("debug"));
    ensure!(
        debug.and_then(toml::Value::as_integer) == Some(0)
            || debug.and_then(toml::Value::as_bool) == Some(false),
        "[profile.dev] must set debug = 0, got `{debug:?}`"
    );

    let config: toml::Value = toml::from_str(&cargo_config()?)?;
    let unstable = config.get("unstable");
    ensure!(
        unstable
            .and_then(|table| table.get("embed-metadata"))
            .and_then(toml::Value::as_bool)
            == Some(false),
        "[unstable] must set embed-metadata = false"
    );
    ensure!(
        unstable
            .and_then(|table| table.get("no-embed-metadata"))
            .is_none(),
        "the old no-embed-metadata key must be absent"
    );
    Ok(())
}
