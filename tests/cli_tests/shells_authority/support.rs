//! Fixtures shared by the `shells` source-authority tests.
//!
//! The scenarios in the parent module read as statements about authority; the
//! mechanics of building a configuration root, driving the merge probe, and
//! phrasing an expectation live here so each scenario stays a few lines long.

use super::super::merge_probe::{isolated_environment, merge_in_child_error};
use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use std::ffi::OsString;
use std::path::Path;
#[cfg(unix)]
use tempfile::tempdir;

/// The smallest definition body accepted by the eventual schema.
pub(super) const SHELLS_TOML: &str =
    "[[shells]]\nname = \"dash\"\nexecutable = \"dash\"\nargs = [\"-c\"]\n";

/// Write `content` into `directory` as `name` through a capability handle.
///
/// The tests construct each configuration root themselves, so the ambient
/// authority is deliberate: the directory being opened is the fixture's own
/// temporary root, not a path supplied by a configuration file.
pub(super) fn write_file(directory: &Path, name: &str, content: &str) -> Result<()> {
    let root = Dir::open_ambient_dir(directory, ambient_authority())
        .with_context(|| format!("open {} to write {name}", directory.display()))?;
    root.write(name, content.as_bytes())
        .with_context(|| format!("write {name} in {}", directory.display()))
}

/// Run a merge probe that must fail and return its reported message.
pub(super) fn rejected_in_child(
    args: &[&str],
    project: &Path,
    home: &Path,
    overrides: &[(OsString, OsString)],
) -> Result<String> {
    let (_xdg_config_dirs, environment) = isolated_environment(home, overrides)?;
    merge_in_child_error(args, project, &environment)
}

/// Assert a rejection names the `shells` key and the authority rule.
pub(super) fn ensure_authority_rejection(message: &str) -> Result<()> {
    ensure!(
        message.contains("shells"),
        "rejection should name the rejected key, got: {message}"
    );
    ensure!(
        message.contains("operator"),
        "rejection should state the operator-authority rule, got: {message}"
    );
    Ok(())
}

/// Write an operator configuration file and return the failure it causes.
///
/// The operator root keeps definition authority, so a failure here is a
/// validation fault rather than an authority fault.
#[cfg(unix)]
pub(super) fn operator_rejection(config: &str) -> Result<String> {
    let project = tempdir().context("create temporary project directory")?;
    let home = tempdir().context("create temporary home directory")?;
    write_file(home.path(), ".netsuke.toml", config)?;
    rejected_in_child(&["netsuke"], project.path(), home.path(), &[])
}
