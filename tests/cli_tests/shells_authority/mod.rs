//! Source-authority tests for the `shells` configuration key.
//!
//! Shell definitions are executable policy, so only operator configuration may
//! declare them. These tests drive the real discovery and merge pipeline in an
//! isolated child process and assert that every other ingress fails closed,
//! including a project chain reached through `extends` and environment
//! variables that would inject the key indirectly.

mod support;

#[cfg(unix)]
use super::merge_probe::{isolated_environment, merge_in_child};
use anyhow::{Context, Result, ensure};
use rstest::rstest;
use std::ffi::OsString;
#[cfg(unix)]
use support::operator_rejection;
use support::{SHELLS_TOML, ensure_authority_rejection, rejected_in_child, write_file};
use tempfile::tempdir;

#[rstest]
fn project_shells_key_is_rejected() -> Result<()> {
    let project = tempdir().context("create temporary project directory")?;
    let home = tempdir().context("create temporary home directory")?;
    write_file(project.path(), ".netsuke.toml", SHELLS_TOML)?;

    let message = rejected_in_child(&["netsuke"], project.path(), home.path(), &[])?;

    ensure_authority_rejection(&message)
}

#[rstest]
fn explicit_shells_file_is_rejected() -> Result<()> {
    let project = tempdir().context("create temporary project directory")?;
    let home = tempdir().context("create temporary home directory")?;
    let explicit_path = project.path().join("operator-lookalike.toml");
    write_file(project.path(), "operator-lookalike.toml", SHELLS_TOML)?;
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
    write_file(
        project.path(),
        "inherited.toml",
        &format!("extends = \"base.toml\"\n{SHELLS_TOML}"),
    )?;
    write_file(project.path(), "base.toml", "jobs = 4\n")?;
    write_file(
        project.path(),
        ".netsuke.toml",
        "extends = \"inherited.toml\"\n",
    )?;

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

/// A valid definition declared by an operator root is accepted.
#[cfg(unix)]
#[rstest]
fn operator_shells_key_is_accepted() -> Result<()> {
    let project = tempdir().context("create temporary project directory")?;
    let home = tempdir().context("create temporary home directory")?;
    write_file(home.path(), ".netsuke.toml", SHELLS_TOML)?;
    let (_xdg_config_dirs, environment) = isolated_environment(home.path(), &[])?;

    merge_in_child(&["netsuke"], project.path(), &environment)?;

    Ok(())
}

/// A field fault is reported against the position and field that caused it.
#[cfg(unix)]
#[rstest]
#[case::built_in_name(
    "[[shells]]\nname = \"bash\"\nexecutable = \"bash\"\nargs = [\"-c\"]\n",
    "shells[0]",
    "built-in"
)]
#[case::relative_executable(
    "[[shells]]\nname = \"dash\"\nexecutable = \"bin/dash\"\nargs = [\"-c\"]\n",
    "shells[0].executable",
    "absolute"
)]
#[case::missing_arguments(
    "[[shells]]\nname = \"dash\"\nexecutable = \"dash\"\n",
    "shells[0].args",
    "required"
)]
#[case::empty_arguments(
    "[[shells]]\nname = \"dash\"\nexecutable = \"dash\"\nargs = []\n",
    "shells[0].args",
    "at least one"
)]
fn operator_shells_field_faults_name_their_position(
    #[case] config: &str,
    #[case] key: &str,
    #[case] phrase: &str,
) -> Result<()> {
    let message = operator_rejection(config)?;

    ensure!(
        message.contains(key),
        "rejection should name {key}, got: {message}"
    );
    ensure!(
        message.contains(phrase),
        "rejection should explain the rule in {phrase:?}, got: {message}"
    );
    Ok(())
}

/// Two operator layers declaring one name are rejected as a collision.
#[cfg(unix)]
#[rstest]
fn operator_layers_declaring_one_name_are_rejected() -> Result<()> {
    let project = tempdir().context("create temporary project directory")?;
    let home = tempdir().context("create temporary home directory")?;
    write_file(home.path(), "layer.toml", SHELLS_TOML)?;
    write_file(
        home.path(),
        ".netsuke.toml",
        &format!("extends = \"layer.toml\"\n{SHELLS_TOML}"),
    )?;

    let message = rejected_in_child(&["netsuke"], project.path(), home.path(), &[])?;

    ensure!(
        message.contains("shells[1]") && message.contains("shells[0]"),
        "collision should name both positions, got: {message}"
    );
    Ok(())
}
