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
use std::path::Path;
use tempfile::tempdir;

/// One sandbox merge described declaratively.
///
/// Naming the configuration file, the environment above it, and the command
/// line in one value lets the success and failure adapters share the same
/// sandbox preparation instead of restating it per shape.
struct MergeCase<'a> {
    /// Configuration file contents written to the temporary home, if any.
    toml: Option<&'a str>,
    /// Environment variables layered above the file layer.
    overrides: &'a [(&'a str, &'a str)],
    /// Arguments handed to the child merge.
    cli_args: &'a [&'a str],
}

impl<'a> MergeCase<'a> {
    /// Merge from the environment alone, with no configuration file.
    fn environment(overrides: &'a [(&'a str, &'a str)]) -> Self {
        Self {
            toml: None,
            overrides,
            cli_args: &["netsuke"],
        }
    }

    /// Merge from a configuration file with no environment overrides.
    fn file(toml: &'a str) -> Self {
        Self {
            toml: Some(toml),
            overrides: &[],
            cli_args: &["netsuke"],
        }
    }

    /// Layer `overrides` above whatever the case already selects.
    fn with_overrides(mut self, overrides: &'a [(&'a str, &'a str)]) -> Self {
        self.overrides = overrides;
        self
    }

    /// Replace the bare `netsuke` argument list with `cli_args`.
    fn with_cli(mut self, cli_args: &'a [&'a str]) -> Self {
        self.cli_args = cli_args;
        self
    }
}

/// Merge the case in an isolated child process.
fn merge(case: &MergeCase<'_>) -> Result<Cli> {
    with_sandbox(case, merge_in_child)
}

/// Merge the case and return the operator-facing rejection message.
fn merge_error(case: &MergeCase<'_>) -> Result<String> {
    with_sandbox(case, merge_in_child_error)
}

/// Prepare the case's sandbox, run `probe`, and keep the sandbox alive until
/// the child process has finished.
fn with_sandbox<T>(
    case: &MergeCase<'_>,
    probe: impl FnOnce(&[&str], &Path, &[(OsString, OsString)]) -> Result<T>,
) -> Result<T> {
    let (xdg, home, environment) = prepare(case)?;
    let outcome = probe(case.cli_args, home.path(), &environment);
    drop(xdg);
    outcome
}

/// The sandbox directories and overrides one merge needs.
///
/// The two directories must outlive the child process, so they travel with the
/// environment rather than being dropped inside the helper.
type Sandbox = (
    tempfile::TempDir,
    tempfile::TempDir,
    Vec<(OsString, OsString)>,
);

/// Write the case's configuration file and build its sandbox environment.
fn prepare(case: &MergeCase<'_>) -> Result<Sandbox> {
    let home = tempdir().context("create temporary home")?;
    let mut overrides = Vec::new();
    if let Some(toml_content) = case.toml {
        let config_path = home.path().join("netsuke.toml");
        std::fs::write(&config_path, toml_content).context("write netsuke.toml")?;
        overrides.push((
            OsString::from("NETSUKE_CONFIG"),
            config_path.into_os_string(),
        ));
    }
    overrides.extend(environment_overrides(case.overrides));
    let (xdg, environment) = isolated_environment(home.path(), &overrides)?;
    Ok((xdg, home, environment))
}

/// Convert string pairs into the owned shape the merge probe expects.
fn environment_overrides(pairs: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
    pairs
        .iter()
        .map(|(key, value)| (OsString::from(*key), OsString::from(*value)))
        .collect()
}

/// Assert that an environment merge fails for one documented rejected form.
fn assert_environment_rejected(key: &str, raw: &str) -> Result<()> {
    let error = merge_error(&MergeCase::environment(&[(key, raw)]))
        .with_context(|| format!("{key}={raw:?} should be rejected by the full merge"))?;
    ensure!(
        !error.is_empty(),
        "{key}={raw:?} should report a non-empty rejection message",
    );
    Ok(())
}
