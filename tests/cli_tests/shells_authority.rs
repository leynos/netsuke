//! Source-authority tests for the `shells` configuration key.
//!
//! Shell definitions are executable policy, so only operator configuration may
//! declare them. These tests drive the real discovery and merge pipeline in an
//! isolated child process and assert that every other ingress fails closed,
//! including a project chain reached through `extends` and environment
//! variables that would inject the key indirectly.

use super::merge_probe::{isolated_environment, merge_in_child_error};
use anyhow::{Context, Result, ensure};
use rstest::rstest;
use std::ffi::OsString;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

/// The smallest definition body accepted by the eventual schema.
const SHELLS_TOML: &str = "[[shells]]\nname = \"dash\"\nexecutable = \"dash\"\nargs = [\"-c\"]\n";

/// Run a merge probe that must fail and return its reported message.
fn rejected_in_child(
    args: &[&str],
    project: &Path,
    home: &Path,
    overrides: &[(OsString, OsString)],
) -> Result<String> {
    let (_xdg_config_dirs, environment) = isolated_environment(home, overrides)?;
    merge_in_child_error(args, project, &environment)
}

/// Assert a rejection names the `shells` key and the authority rule.
fn ensure_authority_rejection(message: &str) -> Result<()> {
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

#[rstest]
fn project_shells_key_is_rejected() -> Result<()> {
    let project = tempdir().context("create temporary project directory")?;
    let home = tempdir().context("create temporary home directory")?;
    fs::write(project.path().join(".netsuke.toml"), SHELLS_TOML)
        .context("write project .netsuke.toml")?;

    let message = rejected_in_child(&["netsuke"], project.path(), home.path(), &[])?;

    ensure_authority_rejection(&message)
}

#[rstest]
fn explicit_shells_file_is_rejected() -> Result<()> {
    let project = tempdir().context("create temporary project directory")?;
    let home = tempdir().context("create temporary home directory")?;
    let explicit_path = project.path().join("operator-lookalike.toml");
    fs::write(&explicit_path, SHELLS_TOML).context("write explicit configuration file")?;
    let explicit = explicit_path.to_string_lossy().into_owned();

    let message = rejected_in_child(
        &["netsuke", "--config", &explicit],
        project.path(),
        home.path(),
        &[],
    )?;

    ensure_authority_rejection(&message)
}

#[rstest]
fn project_parent_shells_is_rejected() -> Result<()> {
    let project = tempdir().context("create temporary project directory")?;
    let home = tempdir().context("create temporary home directory")?;
    fs::write(
        project.path().join("inherited.toml"),
        format!("extends = \"base.toml\"\n{SHELLS_TOML}"),
    )
    .context("write inheriting project configuration")?;
    fs::write(project.path().join("base.toml"), "jobs = 4\n")
        .context("write inherited base configuration")?;
    fs::write(
        project.path().join(".netsuke.toml"),
        "extends = \"inherited.toml\"\n",
    )
    .context("write project .netsuke.toml")?;

    let message = rejected_in_child(&["netsuke"], project.path(), home.path(), &[])?;

    ensure_authority_rejection(&message)
}

#[rstest]
#[case::flat("NETSUKE_SHELLS")]
#[case::nested("NETSUKE_SHELLS__0__NAME")]
#[case::mixed_case("netsuke_Shells")]
fn environment_shells_is_rejected(#[case] variable: &str) -> Result<()> {
    let project = tempdir().context("create temporary project directory")?;
    let home = tempdir().context("create temporary home directory")?;
    let overrides = [(OsString::from(variable), OsString::from("dash"))];

    let message = rejected_in_child(&["netsuke"], project.path(), home.path(), &overrides)?;

    ensure!(
        message.contains("shell") && message.contains("configuration file"),
        "environment rejection should state the file-only rule, got: {message}"
    );
    Ok(())
}

/// An operator root may not declare definitions until the schema exists.
///
/// Weakens nothing: M1 must not silently accept a key it cannot validate, so
/// the operator case fails closed as well.
#[cfg(unix)]
#[rstest]
fn operator_shells_key_is_not_yet_supported() -> Result<()> {
    let project = tempdir().context("create temporary project directory")?;
    let home = tempdir().context("create temporary home directory")?;
    fs::write(home.path().join(".netsuke.toml"), SHELLS_TOML)
        .context("write operator .netsuke.toml")?;

    let message = rejected_in_child(&["netsuke"], project.path(), home.path(), &[])?;

    ensure!(
        message.contains("shells"),
        "operator rejection should name the key, got: {message}"
    );
    Ok(())
}
