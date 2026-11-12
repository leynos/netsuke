//! User-before-system discovery regression matrix.
//!
//! `ortho_config` 0.10.0 probes `XDG_CONFIG_DIRS` before its home fallbacks, so
//! a valid system file can suppress a restrictive user file whenever
//! `XDG_CONFIG_HOME` does not name that user file directly. These cases pin the
//! Netsuke-owned user pass that must consider every user location before any
//! system location.

use super::SYSTEM_CONFIG_CONTENT;
use crate::cli_tests::merge_probe::{
    XdgConfigHomeMode, environment_with_system_scope, environment_with_xdg_mode, merge_in_child,
};
use anyhow::{Context, Result, ensure};
use netsuke::cli::config::{ColourPolicy, EmojiPolicy};
use rstest::rstest;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

/// User configuration file a scenario writes before merging.
#[derive(Clone, Copy, Debug)]
enum UserFileLocation {
    /// `$HOME/.netsuke.toml`, reached through the home dotfile fallback.
    HomeDotfile,
    /// `$HOME/.config/netsuke/config.toml`, reached through the home fallback
    /// when `XDG_CONFIG_HOME` names no usable user directory.
    HomeConfigDirectory,
}

impl UserFileLocation {
    /// Return the configuration path for this location beneath `home`.
    fn path(self, home: &Path) -> PathBuf {
        match self {
            Self::HomeDotfile => home.join(".netsuke.toml"),
            Self::HomeConfigDirectory => home.join(".config/netsuke/config.toml"),
        }
    }
}

/// Write the shared system-scope configuration beneath `system`.
fn write_system_config(system: &Path) -> Result<()> {
    let config_dir = system.join("netsuke");
    fs::create_dir_all(&config_dir)
        .with_context(|| format!("create system config directory {}", config_dir.display()))?;
    fs::write(config_dir.join("config.toml"), SYSTEM_CONFIG_CONTENT).context("write system config")
}

/// Write `contents` to `path`, creating any missing parent directory.
fn write_user_config(path: &Path, contents: &str) -> Result<()> {
    fs::create_dir_all(path.parent().context("user config path has a parent")?)
        .with_context(|| format!("create user config directory for {}", path.display()))?;
    fs::write(path, contents).with_context(|| format!("write user config {}", path.display()))
}

/// Assert that the user layer won and no system-only field merged through.
fn assert_user_precedence_applied(merged: &netsuke::cli::Cli) -> Result<()> {
    ensure!(
        merged.file.as_path() == Path::new("Netsukefile"),
        "manifest path should fall back to the default when the system layer loses, got {:?}",
        merged.file
    );
    ensure!(
        merged.emoji == EmojiPolicy::Never,
        "user config emoji policy should override system config"
    );
    ensure!(
        merged.jobs == Some(4),
        "user config jobs should override system config"
    );
    ensure!(
        merged.color == ColourPolicy::Auto,
        "system color field should not appear when the user layer wins"
    );
    Ok(())
}

/// A user file must outrank a valid system file in every state where
/// `XDG_CONFIG_HOME` names no usable user configuration directory.
///
/// This extends `user_scope_config_takes_precedence_over_system_scope`, which
/// covers only the case where `XDG_CONFIG_HOME` points at the user file.
#[rstest]
#[case::home_dotfile_unset(UserFileLocation::HomeDotfile, XdgConfigHomeMode::Unset)]
#[case::home_dotfile_empty(UserFileLocation::HomeDotfile, XdgConfigHomeMode::Empty)]
#[case::home_dotfile_no_config(UserFileLocation::HomeDotfile, XdgConfigHomeMode::NoConfigFile)]
#[case::home_config_unset(UserFileLocation::HomeConfigDirectory, XdgConfigHomeMode::Unset)]
#[case::home_config_empty(UserFileLocation::HomeConfigDirectory, XdgConfigHomeMode::Empty)]
#[case::home_config_no_config(
    UserFileLocation::HomeConfigDirectory,
    XdgConfigHomeMode::NoConfigFile
)]
fn user_scope_config_takes_precedence_without_usable_xdg_config_home(
    #[case] location: UserFileLocation,
    #[case] mode: XdgConfigHomeMode,
) -> Result<()> {
    let project = tempdir().context("create temporary project directory")?;
    let home = tempdir().context("create temporary home directory")?;
    let system = tempdir().context("create temporary system directory")?;
    write_system_config(system.path())?;
    write_user_config(&location.path(home.path()), "emoji = \"never\"\njobs = 4\n")?;

    let environment = environment_with_xdg_mode(home.path(), system.path(), mode, &[]);
    let merged = merge_in_child(&["netsuke"], project.path(), &environment)?;

    assert_user_precedence_applied(&merged)
}

/// An XDG user file outranks a home dotfile that would otherwise be the first
/// home fallback.
#[rstest]
fn xdg_config_home_user_file_outranks_home_dotfile() -> Result<()> {
    let project = tempdir().context("create temporary project directory")?;
    let home = tempdir().context("create temporary home directory")?;
    let system = tempdir().context("create temporary system directory")?;
    write_system_config(system.path())?;
    write_user_config(
        &home.path().join(".config/netsuke/config.toml"),
        "emoji = \"never\"\njobs = 4\n",
    )?;
    write_user_config(
        &home.path().join(".netsuke.toml"),
        "emoji = \"always\"\njobs = 9\n",
    )?;

    let environment = environment_with_system_scope(home.path(), system.path(), &[]);
    let merged = merge_in_child(&["netsuke"], project.path(), &environment)?;

    ensure!(
        merged.emoji == EmojiPolicy::Never,
        "the XDG user file should outrank the home dotfile"
    );
    ensure!(
        merged.jobs == Some(4),
        "the XDG user file jobs should outrank the home dotfile"
    );
    Ok(())
}

/// A malformed user file stops discovery instead of falling back to a valid
/// system file.
#[rstest]
fn malformed_user_config_stops_discovery_without_system_fallback() -> Result<()> {
    let project = tempdir().context("create temporary project directory")?;
    let home = tempdir().context("create temporary home directory")?;
    let system = tempdir().context("create temporary system directory")?;
    write_system_config(system.path())?;
    let user_config = home.path().join(".config/netsuke/config.toml");
    write_user_config(&user_config, "emoji = \"always\n")?;

    let environment = environment_with_system_scope(home.path(), system.path(), &[]);
    let error = merge_in_child(&["netsuke"], project.path(), &environment)
        .expect_err("a malformed user file must not fall back to system configuration");
    let error_chain = format!("{error:#}");
    ensure!(
        error_chain.contains("config.toml"),
        "the error should name the broken user file: {error_chain}"
    );
    Ok(())
}

/// A user file whose `extends` parent is missing stops discovery instead of
/// falling back to a valid system file.
#[rstest]
fn missing_extends_parent_in_user_config_stops_discovery() -> Result<()> {
    let project = tempdir().context("create temporary project directory")?;
    let home = tempdir().context("create temporary home directory")?;
    let system = tempdir().context("create temporary system directory")?;
    write_system_config(system.path())?;
    let user_config = home.path().join(".config/netsuke/config.toml");
    write_user_config(&user_config, "extends = \"missing-parent.toml\"\n")?;

    let environment = environment_with_system_scope(home.path(), system.path(), &[]);
    let error = merge_in_child(&["netsuke"], project.path(), &environment)
        .expect_err("a missing extends parent must not fall back to system configuration");
    let error_chain = format!("{error:#}");
    ensure!(
        error_chain.contains("missing-parent.toml"),
        "the error should name the missing parent: {error_chain}"
    );
    Ok(())
}

/// When the working directory is the home directory, `$HOME/.netsuke.toml` is
/// both the user configuration and the project configuration.
///
/// The dual role resolves to project scope, so its environment grant stays
/// quarantined rather than being promoted to operator authority. Reading the
/// user scope first must not turn `$HOME` into an operator root: a project
/// configuration's grants are untrusted wherever they are found.
#[rstest]
fn home_dotfile_acting_as_project_config_keeps_project_quarantine() -> Result<()> {
    let home = tempdir().context("create temporary home directory")?;
    let system = tempdir().context("create temporary system directory")?;
    write_system_config(system.path())?;
    write_user_config(
        &home.path().join(".netsuke.toml"),
        "env_allow_var = [\"SAFE\"]\nemoji = \"never\"\n",
    )?;

    let environment =
        environment_with_xdg_mode(home.path(), system.path(), XdgConfigHomeMode::Unset, &[]);
    // The working directory equals the home directory, so automatic project
    // discovery resolves the same physical file as the user base.
    let merged = merge_in_child(&["netsuke"], home.path(), &environment)?;

    // An empty allowlist is default-allow, so only the quarantined field itself
    // shows whether the grant survived.
    ensure!(
        merged.env_allow_var.is_empty(),
        "a project configuration must not contribute an environment grant: {:?}",
        merged.env_allow_var
    );
    ensure!(
        merged.emoji == EmojiPolicy::Never,
        "the user layer should still win over the system layer"
    );
    Ok(())
}
