//! Project- and user-scope configuration discovery tests: automatic
//! project-file discovery, user-scope fallback, and project-over-user
//! precedence on Unix and Windows.

use anyhow::{Context, Result, ensure};
use netsuke::cli::config::{ColourPolicy, EmojiPolicy};
use netsuke::cli_localization;
use rstest::rstest;
#[cfg(unix)]
use std::ffi::OsStr;
use std::fs;
use std::sync::Arc;
use tempfile::tempdir;
use test_support::{EnvVarGuard, env_lock::EnvLock};

use super::CwdGuard;

#[rstest]
fn project_scope_config_discovered_automatically() -> Result<()> {
    let _env_lock = EnvLock::acquire();
    let cwd_guard = CwdGuard::acquire().context("capture current working directory")?;
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

    // Clear env vars that could interfere
    let _config_guard = EnvVarGuard::remove("NETSUKE_CONFIG");
    let _config_path_guard = EnvVarGuard::remove("NETSUKE_CONFIG_PATH");
    let _emoji_guard = EnvVarGuard::remove("NETSUKE_EMOJI");
    let _locale_guard = EnvVarGuard::remove("NETSUKE_LOCALE");
    let _jobs_guard = EnvVarGuard::remove("NETSUKE_JOBS");

    // Change to project directory and parse CLI
    std::env::set_current_dir(&temp_dir).context("change to project directory")?;

    let localizer = Arc::from(cli_localization::build_localizer(None));
    let (cli, matches) = netsuke::cli::parse_with_localizer_from(["netsuke"], &localizer)
        .context("parse CLI for project config discovery")?;
    let merged = netsuke::cli::merge_with_config(&cli, &matches)
        .context("merge with project config")?
        .with_default_command();

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
    drop(cwd_guard);
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

fn run_user_scope_scenario(temp_project: &tempfile::TempDir) -> Result<netsuke::cli::Cli> {
    std::env::set_current_dir(temp_project).context("change to project directory")?;

    let localizer = Arc::from(cli_localization::build_localizer(None));
    let (cli, matches) = netsuke::cli::parse_with_localizer_from(["netsuke"], &localizer)
        .context("parse CLI for user config discovery")?;
    Ok(netsuke::cli::merge_with_config(&cli, &matches)
        .context("merge with user config")?
        .with_default_command())
}

#[cfg(unix)]
#[rstest]
fn user_scope_config_discovered_when_no_project_config() -> Result<()> {
    let _env_lock = EnvLock::acquire();
    let cwd_guard = CwdGuard::acquire().context("capture current working directory")?;
    let temp_project = tempdir().context("create temporary project directory")?;
    let temp_home = tempdir().context("create temporary home directory")?;

    // Write user-scope config in fake home
    fs::write(temp_home.path().join(".netsuke.toml"), USER_CONFIG_CONTENT)
        .context("write user .netsuke.toml")?;

    // Set HOME to fake home (Unix-like systems)
    let _home_guard = EnvVarGuard::set("HOME", temp_home.path().as_os_str());
    // Sandbox XDG paths so system-wide configs cannot leak into the test
    let xdg_config_home = temp_home.path().join(".config");
    fs::create_dir_all(&xdg_config_home).context("create sandboxed XDG_CONFIG_HOME")?;
    let _xdg_config_home_guard = EnvVarGuard::set("XDG_CONFIG_HOME", xdg_config_home.as_os_str());
    let _xdg_config_dirs_guard = EnvVarGuard::set("XDG_CONFIG_DIRS", OsStr::new(""));
    // Clear other env vars
    let _config_guard = EnvVarGuard::remove("NETSUKE_CONFIG");
    let _config_path_guard = EnvVarGuard::remove("NETSUKE_CONFIG_PATH");
    let _emoji_guard = EnvVarGuard::remove("NETSUKE_EMOJI");
    let _jobs_guard = EnvVarGuard::remove("NETSUKE_JOBS");
    let _color_policy_guard = EnvVarGuard::remove("NETSUKE_COLOR");

    let merged = run_user_scope_scenario(&temp_project)?;
    let result = assert_user_config_applied(&merged);
    drop(cwd_guard);
    result
}

#[cfg(windows)]
#[rstest]
fn user_scope_config_discovered_when_no_project_config() -> Result<()> {
    let _env_lock = EnvLock::acquire();
    let cwd_guard = CwdGuard::acquire().context("capture current working directory")?;
    let temp_project = tempdir().context("create temporary project directory")?;
    let temp_appdata = tempdir().context("create temporary APPDATA directory")?;

    // Create netsuke subdirectory in fake APPDATA
    let netsuke_config_dir = temp_appdata.path().join("netsuke");
    fs::create_dir_all(&netsuke_config_dir).context("create netsuke config directory")?;

    // Write user-scope config in fake APPDATA
    fs::write(netsuke_config_dir.join("config.toml"), USER_CONFIG_CONTENT)
        .context("write user config.toml in APPDATA")?;

    // Set APPDATA to fake directory (Windows)
    let _appdata_guard = EnvVarGuard::set("APPDATA", temp_appdata.path().as_os_str());
    // Clear other env vars
    let _config_guard = EnvVarGuard::remove("NETSUKE_CONFIG");
    let _config_path_guard = EnvVarGuard::remove("NETSUKE_CONFIG_PATH");
    let _emoji_guard = EnvVarGuard::remove("NETSUKE_EMOJI");
    let _jobs_guard = EnvVarGuard::remove("NETSUKE_JOBS");
    let _color_policy_guard = EnvVarGuard::remove("NETSUKE_COLOR");
    let _localappdata_guard = EnvVarGuard::remove("LOCALAPPDATA");

    let merged = run_user_scope_scenario(&temp_project)?;
    let result = assert_user_config_applied(&merged);
    drop(cwd_guard);
    result
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

fn run_precedence_scenario(temp_project: &tempfile::TempDir) -> Result<netsuke::cli::Cli> {
    std::env::set_current_dir(temp_project).context("change to project directory")?;

    let localizer = Arc::from(cli_localization::build_localizer(None));
    let (cli, matches) = netsuke::cli::parse_with_localizer_from(["netsuke"], &localizer)
        .context("parse CLI for precedence test")?;
    Ok(netsuke::cli::merge_with_config(&cli, &matches)
        .context("merge configs")?
        .with_default_command())
}

#[cfg(unix)]
#[rstest]
fn project_config_takes_precedence_over_user_config() -> Result<()> {
    let _env_lock = EnvLock::acquire();
    let cwd_guard = CwdGuard::acquire().context("capture current working directory")?;

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
    let _home_guard = EnvVarGuard::set("HOME", temp_home.path().as_os_str());
    let _xdg_home_guard = EnvVarGuard::set("XDG_CONFIG_HOME", temp_xdg_home.path().as_os_str());
    let _xdg_dirs_guard = EnvVarGuard::set("XDG_CONFIG_DIRS", OsStr::new(""));
    let _config_guard = EnvVarGuard::remove("NETSUKE_CONFIG");
    let _config_path_guard = EnvVarGuard::remove("NETSUKE_CONFIG_PATH");
    let _theme_guard = EnvVarGuard::remove("NETSUKE_THEME");
    let _jobs_guard = EnvVarGuard::remove("NETSUKE_JOBS");
    let _colour_guard = EnvVarGuard::remove("NETSUKE_COLOUR_POLICY");

    let merged = run_precedence_scenario(&temp_project)?;
    let result = assert_project_precedence_applied(&merged);
    drop(cwd_guard);
    result
}

#[cfg(windows)]
#[rstest]
fn project_config_takes_precedence_over_user_config() -> Result<()> {
    let _env_lock = EnvLock::acquire();
    let cwd_guard = CwdGuard::acquire().context("capture current working directory")?;

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

    let _appdata_guard = EnvVarGuard::set("APPDATA", temp_appdata.path().as_os_str());
    let _localappdata_guard = EnvVarGuard::remove("LOCALAPPDATA");
    let _config_guard = EnvVarGuard::remove("NETSUKE_CONFIG");
    let _config_path_guard = EnvVarGuard::remove("NETSUKE_CONFIG_PATH");
    let _theme_guard = EnvVarGuard::remove("NETSUKE_THEME");
    let _jobs_guard = EnvVarGuard::remove("NETSUKE_JOBS");
    let _colour_guard = EnvVarGuard::remove("NETSUKE_COLOUR_POLICY");

    let merged = run_precedence_scenario(&temp_project)?;
    let result = assert_project_precedence_applied(&merged);
    drop(cwd_guard);
    result
}
