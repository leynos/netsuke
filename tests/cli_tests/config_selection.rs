//! Integration tests for explicit configuration file selection.
//!
//! These tests cover the visible `--config` flag and `NETSUKE_CONFIG`
//! environment variable, plus compatibility with the legacy
//! `NETSUKE_CONFIG_PATH` override.

use anyhow::{Context, Result, ensure};
use netsuke::cli_localization;
use netsuke::theme::ThemePreference;
use rstest::rstest;
use std::ffi::OsStr;
use std::fs;
use std::sync::Arc;
use tempfile::tempdir;
use test_support::{EnvVarGuard, env_lock::EnvLock};

/// RAII guard that restores the process working directory on drop.
struct CwdGuard(std::path::PathBuf);

impl CwdGuard {
    fn acquire() -> Result<Self> {
        Ok(Self(
            std::env::current_dir().context("capture current working directory")?,
        ))
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        drop(std::env::set_current_dir(&self.0));
    }
}

fn parse_and_merge(args: &[&str]) -> Result<netsuke::cli::Cli> {
    let localizer = Arc::from(cli_localization::build_localizer(None));
    let (cli, matches) = netsuke::cli::parse_with_localizer_from(args, &localizer)
        .context("parse CLI for config selection test")?;
    netsuke::cli::merge_with_config(&cli, &matches)
        .context("merge CLI with selected config")?
        .with_default_command()
        .pipe(Ok)
}

trait Pipe: Sized {
    fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T {
        f(self)
    }
}

impl<T> Pipe for T {}

fn sandbox_user_scope(home: &tempfile::TempDir) -> Result<(EnvVarGuard, EnvVarGuard, EnvVarGuard)> {
    let xdg_config_home = home.path().join(".config");
    fs::create_dir_all(&xdg_config_home).context("create sandboxed XDG config home")?;
    Ok((
        EnvVarGuard::set("HOME", home.path().as_os_str()),
        EnvVarGuard::set("XDG_CONFIG_HOME", xdg_config_home.as_os_str()),
        EnvVarGuard::set("XDG_CONFIG_DIRS", OsStr::new("")),
    ))
}

#[rstest]
fn config_flag_loads_specified_file() -> Result<()> {
    let _env_lock = EnvLock::acquire();
    let _cwd_guard = CwdGuard::acquire()?;
    let project = tempdir().context("create project directory")?;
    let home = tempdir().context("create fake home directory")?;
    let _user_scope = sandbox_user_scope(&home)?;
    let _config_guard = EnvVarGuard::remove("NETSUKE_CONFIG");
    let _legacy_guard = EnvVarGuard::remove("NETSUKE_CONFIG_PATH");
    let _theme_guard = EnvVarGuard::remove("NETSUKE_THEME");

    let custom = project.path().join("custom.toml");
    fs::write(&custom, "theme = \"unicode\"\n").context("write explicit config file")?;
    std::env::set_current_dir(project.path()).context("change to project directory")?;

    let merged = parse_and_merge(&["netsuke", "--config", "custom.toml"])?;
    ensure!(
        merged.theme == Some(ThemePreference::Unicode),
        "explicit --config file should be loaded"
    );
    Ok(())
}

#[rstest]
fn config_flag_skips_project_discovery() -> Result<()> {
    let _env_lock = EnvLock::acquire();
    let _cwd_guard = CwdGuard::acquire()?;
    let project = tempdir().context("create project directory")?;
    let home = tempdir().context("create fake home directory")?;
    let _user_scope = sandbox_user_scope(&home)?;
    let _config_guard = EnvVarGuard::remove("NETSUKE_CONFIG");
    let _legacy_guard = EnvVarGuard::remove("NETSUKE_CONFIG_PATH");

    fs::write(project.path().join(".netsuke.toml"), "theme = \"ascii\"\n")
        .context("write project config")?;
    fs::write(project.path().join("custom.toml"), "theme = \"unicode\"\n")
        .context("write custom config")?;
    std::env::set_current_dir(project.path()).context("change to project directory")?;

    let merged = parse_and_merge(&["netsuke", "--config", "custom.toml"])?;
    ensure!(
        merged.theme == Some(ThemePreference::Unicode),
        "explicit --config should bypass discovered project config"
    );
    Ok(())
}

#[rstest]
fn config_flag_with_nonexistent_file_produces_error() -> Result<()> {
    let _env_lock = EnvLock::acquire();
    let _cwd_guard = CwdGuard::acquire()?;
    let project = tempdir().context("create project directory")?;
    let home = tempdir().context("create fake home directory")?;
    let _user_scope = sandbox_user_scope(&home)?;
    let _config_guard = EnvVarGuard::remove("NETSUKE_CONFIG");
    let _legacy_guard = EnvVarGuard::remove("NETSUKE_CONFIG_PATH");
    std::env::set_current_dir(project.path()).context("change to project directory")?;

    let error = parse_and_merge(&["netsuke", "--config", "missing.toml"])
        .expect_err("missing explicit config file should fail");
    let message = format!("{error:?}");
    ensure!(
        message.contains("missing.toml"),
        "error should mention the missing explicit config path, got {message}"
    );
    Ok(())
}

#[rstest]
fn netsuke_config_env_loads_specified_file() -> Result<()> {
    let _env_lock = EnvLock::acquire();
    let _cwd_guard = CwdGuard::acquire()?;
    let project = tempdir().context("create project directory")?;
    let home = tempdir().context("create fake home directory")?;
    let _user_scope = sandbox_user_scope(&home)?;
    let _legacy_guard = EnvVarGuard::remove("NETSUKE_CONFIG_PATH");

    let custom = project.path().join("env.toml");
    fs::write(&custom, "theme = \"unicode\"\n").context("write NETSUKE_CONFIG file")?;
    let _config_guard = EnvVarGuard::set("NETSUKE_CONFIG", custom.as_os_str());
    std::env::set_current_dir(project.path()).context("change to project directory")?;

    let merged = parse_and_merge(&["netsuke"])?;
    ensure!(
        merged.theme == Some(ThemePreference::Unicode),
        "NETSUKE_CONFIG should load the selected config file"
    );
    Ok(())
}

#[rstest]
fn netsuke_config_env_takes_precedence_over_legacy() -> Result<()> {
    let _env_lock = EnvLock::acquire();
    let _cwd_guard = CwdGuard::acquire()?;
    let project = tempdir().context("create project directory")?;
    let home = tempdir().context("create fake home directory")?;
    let _user_scope = sandbox_user_scope(&home)?;

    let new_config = project.path().join("new.toml");
    let legacy_config = project.path().join("legacy.toml");
    fs::write(&new_config, "theme = \"unicode\"\n").context("write NETSUKE_CONFIG file")?;
    fs::write(&legacy_config, "theme = \"ascii\"\n").context("write legacy config file")?;
    let _config_guard = EnvVarGuard::set("NETSUKE_CONFIG", new_config.as_os_str());
    let _legacy_guard = EnvVarGuard::set("NETSUKE_CONFIG_PATH", legacy_config.as_os_str());
    std::env::set_current_dir(project.path()).context("change to project directory")?;

    let merged = parse_and_merge(&["netsuke"])?;
    ensure!(
        merged.theme == Some(ThemePreference::Unicode),
        "NETSUKE_CONFIG should win over NETSUKE_CONFIG_PATH"
    );
    Ok(())
}

#[rstest]
fn config_flag_takes_precedence_over_netsuke_config_env() -> Result<()> {
    let _env_lock = EnvLock::acquire();
    let _cwd_guard = CwdGuard::acquire()?;
    let project = tempdir().context("create project directory")?;
    let home = tempdir().context("create fake home directory")?;
    let _user_scope = sandbox_user_scope(&home)?;

    let cli_config = project.path().join("cli.toml");
    let env_config = project.path().join("env.toml");
    fs::write(&cli_config, "theme = \"unicode\"\n").context("write CLI-selected config")?;
    fs::write(&env_config, "theme = \"ascii\"\n").context("write env-selected config")?;
    let _config_guard = EnvVarGuard::set("NETSUKE_CONFIG", env_config.as_os_str());
    let _legacy_guard = EnvVarGuard::remove("NETSUKE_CONFIG_PATH");
    std::env::set_current_dir(project.path()).context("change to project directory")?;

    let merged = parse_and_merge(&["netsuke", "--config", "cli.toml"])?;
    ensure!(
        merged.theme == Some(ThemePreference::Unicode),
        "--config should win over NETSUKE_CONFIG"
    );
    Ok(())
}

#[rstest]
fn config_flag_values_still_overridden_by_env_and_cli_preferences() -> Result<()> {
    let _env_lock = EnvLock::acquire();
    let _cwd_guard = CwdGuard::acquire()?;
    let project = tempdir().context("create project directory")?;
    let home = tempdir().context("create fake home directory")?;
    let _user_scope = sandbox_user_scope(&home)?;
    let _legacy_guard = EnvVarGuard::remove("NETSUKE_CONFIG_PATH");
    let _theme_guard = EnvVarGuard::set("NETSUKE_THEME", "unicode");

    let custom = project.path().join("custom.toml");
    fs::write(&custom, "theme = \"ascii\"\n").context("write explicit config file")?;
    std::env::set_current_dir(project.path()).context("change to project directory")?;

    let merged_with_cli_override =
        parse_and_merge(&["netsuke", "--config", "custom.toml", "--theme", "ascii"])?;
    ensure!(
        merged_with_cli_override.theme == Some(ThemePreference::Ascii),
        "CLI preference values should still override environment and selected config"
    );

    let merged_with_env_override = parse_and_merge(&["netsuke", "--config", "custom.toml"])?;
    ensure!(
        merged_with_env_override.theme == Some(ThemePreference::Unicode),
        "environment preference values should still override the selected config"
    );
    Ok(())
}
