//! Contracts for the build footprint rule's settings.
//!
//! The Concordat `rust-dev-build-footprint` rule needs the dev profile to
//! disable incremental compilation and debug information, and the pinned
//! nightly to take Cargo's native `embed-metadata = false` rather than a raw
//! rustc flag. These tests keep an edit to `Cargo.toml` or
//! `.cargo/config.toml` from quietly undoing any of the three.
#![cfg(all(unix, target_os = "linux"))]

use anyhow::{Context, Result, ensure};
use std::process::Command;
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

/// Ask Cargo for the unit graph of a `cargo` subcommand and check the profile
/// every unit carries.
///
/// `--unit-graph` resolves the workspace and prints the units Cargo would
/// build without compiling anything, so this reads Cargo's own resolved view
/// of the profiles, including the test profile that inherits from `dev`, at
/// the cost of a metadata read rather than compiler work inside a timed test.
fn assert_units_have_no_debuginfo_or_incremental(subcommand: &[&str]) -> Result<()> {
    let output = Command::new(env!("CARGO"))
        .args(subcommand)
        .args([
            "--unit-graph",
            "-Z",
            "unstable-options",
            "--locked",
            "--offline",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .context("failed to run cargo --unit-graph")?;
    ensure!(
        output.status.success(),
        "cargo {subcommand:?} --unit-graph failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let graph: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let units = graph
        .get("units")
        .and_then(serde_json::Value::as_array)
        .context("the unit graph has no units")?;
    ensure!(!units.is_empty(), "the unit graph is empty");
    for unit in units {
        let profile = unit.get("profile").context("a unit has no profile")?;
        let name = unit.get("target").and_then(|target| target.get("name"));
        ensure!(
            profile.get("debuginfo").and_then(serde_json::Value::as_u64) == Some(0),
            "unit {name:?} carries debuginfo {:?}, want 0",
            profile.get("debuginfo")
        );
        ensure!(
            profile
                .get("incremental")
                .and_then(serde_json::Value::as_bool)
                == Some(false),
            "unit {name:?} builds incrementally"
        );
    }
    Ok(())
}

/// The default development build carries no debug information and no
/// incremental state in any unit.
#[test]
fn the_dev_unit_graph_has_no_debuginfo_or_incremental_state() -> Result<()> {
    assert_units_have_no_debuginfo_or_incremental(&["build"])
}

/// The test profile inherits both settings from the dev profile.
#[test]
fn the_test_unit_graph_has_no_debuginfo_or_incremental_state() -> Result<()> {
    assert_units_have_no_debuginfo_or_incremental(&["test", "--no-run"])
}
