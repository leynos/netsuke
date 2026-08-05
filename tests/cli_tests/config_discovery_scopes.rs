//! Project- and user-scope configuration discovery tests: automatic
//! project-file discovery, user-scope fallback, and project-over-user
//! precedence on Unix and Windows.

use super::super::merge_probe::{isolated_environment, merge_in_child};
use anyhow::{Context, Result, ensure};
use netsuke::cli::config::{ColourPolicy, EmojiPolicy};
use rstest::rstest;
use std::ffi::OsString;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

fn run_scope_scenario(
    project: &Path,
    home: &Path,
    overrides: &[(OsString, OsString)],
) -> Result<netsuke::cli::Cli> {
    let (_xdg_config_dirs, environment) = isolated_environment(home, overrides)?;
    merge_in_child(&["netsuke"], project, &environment)
}

#[rstest]
fn project_scope_config_discovered_automatically() -> Result<()> {
    let temp_dir = tempdir().context("create temporary project directory")?;
    let project_config = temp_dir.path().join(".netsuke.toml");

    // Write project-scope config
    fs::write(
        &project_config,
        r#"
emoji = "always"
locale = "es-ES"
jobs = 8
"#,
    )
    .context("write project .netsuke.toml")?;

    let merged = run_scope_scenario(temp_dir.path(), temp_dir.path(), &[])?;

    ensure!(
        merged.emoji == EmojiPolicy::Always,
        "project config emoji policy should be discovered and applied"
    );
    ensure!(
        merged.locale.as_deref() == Some("es-ES"),
        "project config locale should be discovered"
    );
    ensure!(
        merged.jobs == Some(8),
        "project config jobs should be discovered"
    );
    Ok(())
}

/// User-scope config content shared by the Unix and Windows test variants.
const USER_CONFIG_CONTENT: &str = r#"
emoji = "never"
color = "never"
jobs = 4
"#;

fn assert_user_config_applied(merged: &netsuke::cli::Cli) -> Result<()> {
    ensure!(
        merged.emoji == EmojiPolicy::Never,
        "user config emoji policy should be discovered when no project config exists"
    );
    ensure!(
        merged.color == ColourPolicy::Never,
        "user config color policy should be discovered"
    );
    ensure!(
        merged.jobs == Some(4),
        "user config jobs should be discovered"
    );
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn user_scope_config_discovered_when_no_project_config() -> Result<()> {
    let temp_project = tempdir().context("create temporary project directory")?;
    let temp_home = tempdir().context("create temporary home directory")?;

    // Write user-scope config in fake home
    fs::write(temp_home.path().join(".netsuke.toml"), USER_CONFIG_CONTENT)
        .context("write user .netsuke.toml")?;

    // Sandbox XDG paths so system-wide configs cannot leak into the test
    let xdg_config_home = temp_home.path().join(".config");
    fs::create_dir_all(&xdg_config_home).context("create sandboxed XDG_CONFIG_HOME")?;
    let merged = run_scope_scenario(
        temp_project.path(),
        temp_home.path(),
        &[(
            OsString::from("XDG_CONFIG_HOME"),
            xdg_config_home.into_os_string(),
        )],
    )?;
    assert_user_config_applied(&merged)
}

#[cfg(windows)]
#[rstest]
fn user_scope_config_discovered_when_no_project_config() -> Result<()> {
    let temp_project = tempdir().context("create temporary project directory")?;
    let temp_appdata = tempdir().context("create temporary APPDATA directory")?;

    // Create netsuke subdirectory in fake APPDATA
    let netsuke_config_dir = temp_appdata.path().join("netsuke");
    fs::create_dir_all(&netsuke_config_dir).context("create netsuke config directory")?;

    // Write user-scope config in fake APPDATA
    fs::write(netsuke_config_dir.join("config.toml"), USER_CONFIG_CONTENT)
        .context("write user config.toml in APPDATA")?;

    // Set APPDATA to fake directory (Windows)
    let merged = run_scope_scenario(
        temp_project.path(),
        temp_project.path(),
        &[(
            OsString::from("APPDATA"),
            temp_appdata.path().as_os_str().to_owned(),
        )],
    )?;
    assert_user_config_applied(&merged)
}

/// Project config TOML used by both Unix and Windows precedence test variants.
const PRECEDENCE_PROJECT_CONFIG_CONTENT: &str = r#"
emoji = "always"
jobs = 8
"#;

/// User config TOML used by both Unix and Windows precedence test variants.
const PRECEDENCE_USER_CONFIG_CONTENT: &str = r#"
emoji = "never"
color = "never"
"#;

fn assert_project_precedence_applied(merged: &netsuke::cli::Cli) -> Result<()> {
    ensure!(
        merged.emoji == EmojiPolicy::Always,
        "project config emoji policy should override user config"
    );
    ensure!(
        merged.jobs == Some(8),
        "project config jobs should be applied"
    );
    ensure!(
        merged.color == ColourPolicy::Never,
        "user-only field should still be merged when project config does not override it"
    );
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn project_config_takes_precedence_over_user_config() -> Result<()> {
    let temp_project = tempdir().context("create temporary project directory")?;
    let temp_home = tempdir().context("create temporary home directory")?;

    // User config: sets theme and a user-only field (colour_policy).
    fs::write(
        temp_home.path().join(".netsuke.toml"),
        PRECEDENCE_USER_CONFIG_CONTENT,
    )
    .context("write user .netsuke.toml")?;

    // Project config: overrides theme; does NOT set colour_policy.
    fs::write(
        temp_project.path().join(".netsuke.toml"),
        PRECEDENCE_PROJECT_CONFIG_CONTENT,
    )
    .context("write project .netsuke.toml")?;

    let temp_xdg_home = tempdir().context("create temporary XDG config home")?;
    let merged = run_scope_scenario(
        temp_project.path(),
        temp_home.path(),
        &[(
            OsString::from("XDG_CONFIG_HOME"),
            temp_xdg_home.path().as_os_str().to_owned(),
        )],
    )?;
    assert_project_precedence_applied(&merged)
}

#[cfg(windows)]
#[rstest]
fn project_config_takes_precedence_over_user_config() -> Result<()> {
    let temp_project = tempdir().context("create temporary project directory")?;
    let temp_appdata = tempdir().context("create temporary APPDATA directory")?;

    // Create sandboxed Windows user-scope config at %APPDATA%\netsuke\config.toml
    let netsuke_config_dir = temp_appdata.path().join("netsuke");
    fs::create_dir_all(&netsuke_config_dir).context("create netsuke config directory")?;
    fs::write(
        netsuke_config_dir.join("config.toml"),
        PRECEDENCE_USER_CONFIG_CONTENT,
    )
    .context("write user config.toml in APPDATA")?;

    // Project config: overrides theme; does NOT set colour_policy.
    fs::write(
        temp_project.path().join(".netsuke.toml"),
        PRECEDENCE_PROJECT_CONFIG_CONTENT,
    )
    .context("write project .netsuke.toml")?;

    let merged = run_scope_scenario(
        temp_project.path(),
        temp_project.path(),
        &[(
            OsString::from("APPDATA"),
            temp_appdata.path().as_os_str().to_owned(),
        )],
    )?;
    assert_project_precedence_applied(&merged)
}
