//! Source-authority tests for the `shells` configuration key.
//!
//! Shell definitions are executable policy, so only operator configuration may
//! declare them. These tests drive the real discovery and merge pipeline in an
//! isolated child process and assert that every other ingress fails closed,
//! including a project chain reached through `extends` and environment
//! variables that would inject the key indirectly.

use super::merge_probe::{isolated_environment, merge_in_child, merge_in_child_error};
use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use rstest::rstest;
use std::ffi::OsString;
use std::path::Path;
use tempfile::tempdir;

/// The smallest definition body accepted by the eventual schema.
const SHELLS_TOML: &str = "[[shells]]\nname = \"dash\"\nexecutable = \"dash\"\nargs = [\"-c\"]\n";

/// Write `content` into `directory` as `name` through a capability handle.
///
/// The tests construct each configuration root themselves, so the ambient
/// authority is deliberate: the directory being opened is the fixture's own
/// temporary root, not a path supplied by a configuration file.
fn write_file(directory: &Path, name: &str, content: &str) -> Result<()> {
    let root = Dir::open_ambient_dir(directory, ambient_authority())
        .with_context(|| format!("open {} to write {name}", directory.display()))?;
    root.write(name, content.as_bytes())
        .with_context(|| format!("write {name} in {}", directory.display()))
}

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

/// Write an operator configuration file and return the failure it causes.
///
/// The operator root keeps definition authority, so a failure here is a
/// validation fault rather than an authority fault.
#[cfg(unix)]
fn operator_rejection(config: &str) -> Result<String> {
    let project = tempdir().context("create temporary project directory")?;
    let home = tempdir().context("create temporary home directory")?;
    write_file(home.path(), ".netsuke.toml", config)?;
    rejected_in_child(&["netsuke"], project.path(), home.path(), &[])
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
