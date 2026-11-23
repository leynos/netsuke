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
use netsuke::cli::Cli;
use netsuke::cli::config::{ColourPolicy, EmojiPolicy};
use rstest::rstest;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::{TempDir, tempdir};

/// User configuration file a scenario writes before merging.
#[derive(Clone, Copy, Debug)]
enum UserFileLocation {
    /// `$HOME/.netsuke.toml`, reached through the home dotfile fallback.
    Dotfile,
    /// `$HOME/.config/netsuke/config.toml`, reached through the home fallback
    /// when `XDG_CONFIG_HOME` names no usable user directory.
    ConfigDirectory,
    /// `$HOME/.config/.netsuke.toml`, the dotfile spelling within the home
    /// `.config` base.
    ConfigDotfile,
}

impl UserFileLocation {
    /// Return the configuration path for this location beneath `home`.
    fn path(self, home: &Path) -> PathBuf {
        match self {
            Self::Dotfile => home.join(".netsuke.toml"),
            Self::ConfigDirectory => home.join(".config/netsuke/config.toml"),
            Self::ConfigDotfile => home.join(".config/.netsuke.toml"),
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

/// Isolated project, home, and system roots for one discovery scenario.
///
/// Every scenario starts with a valid system file installed, so a case that
/// passes only because the system layer went unread is visible as the wrong
/// layer supplying the merged values.
struct Scenario {
    project: TempDir,
    home: TempDir,
    system: TempDir,
}

impl Scenario {
    /// Create three isolated roots and install the shared system configuration.
    fn new() -> Result<Self> {
        let project = tempdir().context("create temporary project directory")?;
        let home = tempdir().context("create temporary home directory")?;
        let system = tempdir().context("create temporary system directory")?;
        write_system_config(system.path())?;
        Ok(Self {
            project,
            home,
            system,
        })
    }

    /// Build an environment whose system scope is this scenario's system root.
    fn system_scope_environment(&self) -> Vec<(OsString, OsString)> {
        environment_with_system_scope(self.home.path(), self.system.path(), &[])
    }

    /// Merge configuration in a child process rooted at the scenario project.
    fn merge(&self, environment: &[(OsString, OsString)]) -> Result<Cli> {
        merge_in_child(&["netsuke"], self.project.path(), environment)
    }
}

/// Assert that the user layer won and no system-only field merged through.
fn assert_user_precedence_applied(merged: &Cli) -> Result<()> {
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
#[case::home_dotfile_unset(UserFileLocation::Dotfile, XdgConfigHomeMode::Unset)]
#[case::home_dotfile_empty(UserFileLocation::Dotfile, XdgConfigHomeMode::Empty)]
#[case::home_dotfile_no_config(UserFileLocation::Dotfile, XdgConfigHomeMode::NoConfigFile)]
#[case::home_config_unset(UserFileLocation::ConfigDirectory, XdgConfigHomeMode::Unset)]
#[case::home_config_empty(UserFileLocation::ConfigDirectory, XdgConfigHomeMode::Empty)]
#[case::home_config_no_config(UserFileLocation::ConfigDirectory, XdgConfigHomeMode::NoConfigFile)]
#[case::home_config_dotfile_unset(UserFileLocation::ConfigDotfile, XdgConfigHomeMode::Unset)]
#[case::home_config_dotfile_empty(UserFileLocation::ConfigDotfile, XdgConfigHomeMode::Empty)]
#[case::home_config_dotfile_no_config(
    UserFileLocation::ConfigDotfile,
    XdgConfigHomeMode::NoConfigFile
)]
fn user_scope_config_takes_precedence_without_usable_xdg_config_home(
    #[case] location: UserFileLocation,
    #[case] mode: XdgConfigHomeMode,
) -> Result<()> {
    let scenario = Scenario::new()?;
    write_user_config(
        &location.path(scenario.home.path()),
        "emoji = \"never\"\njobs = 4\n",
    )?;

    let environment =
        environment_with_xdg_mode(scenario.home.path(), scenario.system.path(), mode, &[]);
    let merged = scenario.merge(&environment)?;

    assert_user_precedence_applied(&merged)
}

/// An XDG user file outranks a home dotfile that would otherwise be the first
/// home fallback.
#[rstest]
fn xdg_config_home_user_file_outranks_home_dotfile() -> Result<()> {
    let scenario = Scenario::new()?;
    write_user_config(
        &scenario.home.path().join(".config/netsuke/config.toml"),
        "emoji = \"never\"\njobs = 4\n",
    )?;
    write_user_config(
        &scenario.home.path().join(".netsuke.toml"),
        "emoji = \"always\"\njobs = 9\n",
    )?;

    let environment = scenario.system_scope_environment();
    let merged = scenario.merge(&environment)?;

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

/// A user file that cannot be loaded stops discovery instead of falling back to
/// a valid system file.
///
/// The first case is syntactically invalid TOML; the second names an `extends`
/// parent that does not exist. Both must surface the user file's own failure,
/// because accepting the system file would silently reinstate the fetch and
/// environment policy the user tried to override.
#[rstest]
#[case::malformed("emoji = \"always\n", "config.toml")]
#[case::missing_extends("extends = \"missing-parent.toml\"\n", "missing-parent.toml")]
fn unloadable_user_config_stops_discovery_without_system_fallback(
    #[case] contents: &str,
    #[case] expected_fragment: &str,
) -> Result<()> {
    let scenario = Scenario::new()?;
    let user_config = scenario.home.path().join(".config/netsuke/config.toml");
    write_user_config(&user_config, contents)?;

    let environment = scenario.system_scope_environment();
    let error = scenario
        .merge(&environment)
        .expect_err("an unloadable user file must not fall back to system configuration");
    let error_chain = format!("{error:#}");
    ensure!(
        error_chain.contains(expected_fragment),
        "the error should name {expected_fragment}: {error_chain}"
    );
    Ok(())
}

/// A user file that exists but does not resolve stops discovery instead of
/// falling back to a valid system file.
///
/// `OrthoConfig` reports a symbolic link whose target is missing as an absent
/// file, so without a separate presence probe the link would be skipped and the
/// system file would reinstate the policy the user meant to override.
#[cfg(unix)]
#[rstest]
fn dangling_user_config_symlink_stops_discovery_without_system_fallback() -> Result<()> {
    let scenario = Scenario::new()?;
    let link = scenario.home.path().join(".netsuke.toml");
    // The target is relative, so the link stays inside the home directory and
    // the loader reports it as an absent file rather than refusing it as a
    // path escape.
    std::os::unix::fs::symlink("absent-target.toml", &link)
        .context("create dangling user configuration symlink")?;

    let environment = scenario.system_scope_environment();
    let error = scenario
        .merge(&environment)
        .expect_err("a dangling user file must not fall back to system configuration");
    let error_chain = format!("{error:#}");
    ensure!(
        error_chain.contains(".netsuke.toml"),
        "the error should name the dangling user configuration: {error_chain}"
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
    let scenario = Scenario::new()?;
    write_user_config(
        &scenario.home.path().join(".netsuke.toml"),
        "env_allow_var = [\"SAFE\"]\nemoji = \"never\"\n",
    )?;

    let environment = environment_with_xdg_mode(
        scenario.home.path(),
        scenario.system.path(),
        XdgConfigHomeMode::Unset,
        &[],
    );
    // The working directory equals the home directory, so automatic project
    // discovery resolves the same physical file as the user base.
    let merged = merge_in_child(&["netsuke"], scenario.home.path(), &environment)?;

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

/// A relative `XDG_CONFIG_HOME` names no base in either discovery pass.
///
/// The child runs in a working directory that holds
/// `relative-base/netsuke/config.toml`, so an implementation that resolved the
/// relative value against the working directory would load that file and its
/// policy. The system file must supply the merged values instead, because a
/// relative base is not a user location.
#[rstest]
fn relative_xdg_config_home_ignores_the_working_directory() -> Result<()> {
    let scenario = Scenario::new()?;
    write_user_config(
        &scenario
            .project
            .path()
            .join("relative-base/netsuke/config.toml"),
        "file = \"Relativefile\"\nemoji = \"never\"\ncolor = \"never\"\njobs = 4\n",
    )?;

    let environment = environment_with_system_scope(
        scenario.home.path(),
        scenario.system.path(),
        &[(
            OsString::from("XDG_CONFIG_HOME"),
            OsString::from("relative-base"),
        )],
    );
    let merged = scenario.merge(&environment)?;

    ensure!(
        merged.file.as_path() == Path::new("Systemfile"),
        "a working-directory relative base must not supply the manifest path, got {:?}",
        merged.file
    );
    ensure!(
        merged.emoji == EmojiPolicy::Always,
        "a working-directory relative base must not supply the emoji policy"
    );
    ensure!(
        merged.jobs == Some(9),
        "the system layer should supply jobs, got {:?}",
        merged.jobs
    );
    Ok(())
}
