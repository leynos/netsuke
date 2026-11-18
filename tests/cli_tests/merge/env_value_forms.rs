//! Full-merge coverage for the documented environment value forms.
//!
//! Issue #1147 reported three documented forms that the early diagnostic pass
//! accepted and the full merge rejected. These tests exercise the real merge
//! boundary for each form: strict Boolean spellings (`1` and `0`), mixed-case
//! policy values, and the supported list shapes. The matching early-pass tests
//! live in `merge_diag.rs`; together they pin that both passes accept exactly
//! the same values.

mod boolean_values;
mod list_values;
mod policy_values;

use super::super::merge_probe::{isolated_environment, merge_in_child, merge_in_child_error};
use anyhow::{Context, Result, ensure};
use netsuke::cli::Cli;
use std::ffi::OsString;
use tempfile::tempdir;

/// Convert string pairs into the owned shape the merge probe expects.
fn environment_overrides(pairs: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
    pairs
        .iter()
        .map(|(key, value)| (OsString::from(*key), OsString::from(*value)))
        .collect()
}

/// Merge configuration from an isolated home with the supplied environment.
fn merge_with_environment(pairs: &[(&str, &str)], cli_args: &[&str]) -> Result<Cli> {
    let home = tempdir().context("create temporary home")?;
    let (xdg, environment) = isolated_environment(home.path(), &environment_overrides(pairs))?;
    let merged = merge_in_child(cli_args, home.path(), &environment);
    drop(xdg);
    merged
}

/// Merge configuration and return the operator-facing rejection message.
fn merge_with_environment_error(pairs: &[(&str, &str)], cli_args: &[&str]) -> Result<String> {
    let home = tempdir().context("create temporary home")?;
    let (xdg, environment) = isolated_environment(home.path(), &environment_overrides(pairs))?;
    let error = merge_in_child_error(cli_args, home.path(), &environment);
    drop(xdg);
    error
}

/// Merge a TOML file together with the supplied environment.
fn merge_with_file_and_environment(
    toml_content: &str,
    pairs: &[(&str, &str)],
    cli_args: &[&str],
) -> Result<Cli> {
    let (xdg, home, environment) = file_environment(toml_content, pairs)?;
    let merged = merge_in_child(cli_args, home.path(), &environment);
    drop(xdg);
    merged
}

/// Merge a TOML file with the environment and return the rejection message.
fn merge_with_file_and_environment_error(
    toml_content: &str,
    pairs: &[(&str, &str)],
    cli_args: &[&str],
) -> Result<String> {
    let (xdg, home, environment) = file_environment(toml_content, pairs)?;
    let error = merge_in_child_error(cli_args, home.path(), &environment);
    drop(xdg);
    error
}

/// Write `toml_content` and build the sandbox environment selecting it.
fn file_environment(
    toml_content: &str,
    pairs: &[(&str, &str)],
) -> Result<(
    tempfile::TempDir,
    tempfile::TempDir,
    Vec<(OsString, OsString)>,
)> {
    let home = tempdir().context("create temporary home")?;
    let config_path = home.path().join("netsuke.toml");
    std::fs::write(&config_path, toml_content).context("write netsuke.toml")?;
    let mut overrides = vec![(
        OsString::from("NETSUKE_CONFIG"),
        config_path.into_os_string(),
    )];
    overrides.extend(environment_overrides(pairs));
    let (xdg, environment) = isolated_environment(home.path(), &overrides)?;
    Ok((xdg, home, environment))
}

/// Assert that an environment merge fails for one documented rejected form.
fn assert_environment_rejected(key: &str, raw: &str) -> Result<()> {
    let error = merge_with_environment_error(&[(key, raw)], &["netsuke"])
        .with_context(|| format!("{key}={raw:?} should be rejected by the full merge"))?;
    ensure!(
        !error.is_empty(),
        "{key}={raw:?} should report a non-empty rejection message",
    );
    Ok(())
}
