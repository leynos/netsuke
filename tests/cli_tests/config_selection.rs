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

struct ConfigTestHarness {
    // Struct fields drop in declaration order; keep the lock last so process
    // state is restored before another test can acquire `EnvLock`.
    _cwd_guard: CwdGuard,
    _user_scope: (EnvVarGuard, EnvVarGuard, EnvVarGuard),
    _home: tempfile::TempDir,
    project: tempfile::TempDir,
    _env_lock: EnvLock,
}

impl ConfigTestHarness {
    fn setup() -> Result<Self> {
        let env_lock = EnvLock::acquire();
        let cwd_guard = CwdGuard::acquire()?;
        let project = tempdir().context("create project directory")?;
        let home = tempdir().context("create fake home directory")?;
        let user_scope = sandbox_user_scope(&home)?;
        std::env::set_current_dir(project.path()).context("change to project directory")?;
        Ok(Self {
            _env_lock: env_lock,
            project,
            _home: home,
            _user_scope: user_scope,
            _cwd_guard: cwd_guard,
        })
    }

    fn write_config(&self, name: &str, content: &str) -> Result<std::path::PathBuf> {
        let path = self.project.path().join(name);
        fs::write(&path, content).with_context(|| format!("write config file {name}"))?;
        std::env::set_current_dir(self.project.path()).context("change to project directory")?;
        Ok(path)
    }
}

#[rstest]
fn config_flag_loads_specified_file() -> Result<()> {
    let h = ConfigTestHarness::setup()?;
    let _config_guard = EnvVarGuard::remove("NETSUKE_CONFIG");
    let _legacy_guard = EnvVarGuard::remove("NETSUKE_CONFIG_PATH");
    let _theme_guard = EnvVarGuard::remove("NETSUKE_THEME");

    let custom_path = h.write_config("custom.toml", "theme = \"unicode\"\n")?;
    let custom_arg = custom_path.to_string_lossy().into_owned();

    let merged = parse_and_merge(&["netsuke", "--config", &custom_arg])?;
    ensure!(
        merged.theme == Some(ThemePreference::Unicode),
        "explicit --config file should be loaded"
    );
    let _project_root = h.project.path();
    Ok(())
}

#[rstest]
fn config_flag_skips_project_discovery() -> Result<()> {
    let h = ConfigTestHarness::setup()?;
    let _config_guard = EnvVarGuard::remove("NETSUKE_CONFIG");
    let _legacy_guard = EnvVarGuard::remove("NETSUKE_CONFIG_PATH");

    let _project_config = h.write_config(".netsuke.toml", "theme = \"ascii\"\n")?;
    let custom_path = h.write_config("custom.toml", "theme = \"unicode\"\n")?;
    let custom_arg = custom_path.to_string_lossy().into_owned();

    let merged = parse_and_merge(&["netsuke", "--config", &custom_arg])?;
    ensure!(
        merged.theme == Some(ThemePreference::Unicode),
        "explicit --config should bypass discovered project config"
    );
    let _project_root = h.project.path();
    Ok(())
}

#[rstest]
fn config_flag_with_nonexistent_file_produces_error() -> Result<()> {
    let h = ConfigTestHarness::setup()?;
    let _config_guard = EnvVarGuard::remove("NETSUKE_CONFIG");
    let _legacy_guard = EnvVarGuard::remove("NETSUKE_CONFIG_PATH");

    let error = parse_and_merge(&["netsuke", "--config", "missing.toml"])
        .expect_err("missing explicit config file should fail");
    let message = format!("{error:?}");
    ensure!(
        message.contains("missing.toml"),
        "error should mention the missing explicit config path, got {message}"
    );
    let _project_root = h.project.path();
    Ok(())
}

#[rstest]
fn netsuke_config_env_loads_specified_file() -> Result<()> {
    let h = ConfigTestHarness::setup()?;
    let _legacy_guard = EnvVarGuard::remove("NETSUKE_CONFIG_PATH");

    let custom = h.write_config("env.toml", "theme = \"unicode\"\n")?;
    let _config_guard = EnvVarGuard::set("NETSUKE_CONFIG", custom.as_os_str());

    let merged = parse_and_merge(&["netsuke"])?;
    ensure!(
        merged.theme == Some(ThemePreference::Unicode),
        "NETSUKE_CONFIG should load the selected config file"
    );
    let _project_root = h.project.path();
    Ok(())
}

#[rstest]
fn netsuke_config_env_takes_precedence_over_legacy() -> Result<()> {
    let h = ConfigTestHarness::setup()?;

    let new_config = h.write_config("new.toml", "theme = \"unicode\"\n")?;
    let legacy_config = h.write_config("legacy.toml", "theme = \"ascii\"\n")?;
    let _config_guard = EnvVarGuard::set("NETSUKE_CONFIG", new_config.as_os_str());
    let _legacy_guard = EnvVarGuard::set("NETSUKE_CONFIG_PATH", legacy_config.as_os_str());

    let merged = parse_and_merge(&["netsuke"])?;
    ensure!(
        merged.theme == Some(ThemePreference::Unicode),
        "NETSUKE_CONFIG should win over NETSUKE_CONFIG_PATH"
    );
    let _project_root = h.project.path();
    Ok(())
}

#[rstest]
fn config_flag_takes_precedence_over_netsuke_config_env() -> Result<()> {
    let h = ConfigTestHarness::setup()?;

    let cli_config_path = h.write_config("cli.toml", "theme = \"unicode\"\n")?;
    let cli_config_arg = cli_config_path.to_string_lossy().into_owned();
    let env_config = h.write_config("env.toml", "theme = \"ascii\"\n")?;
    let _config_guard = EnvVarGuard::set("NETSUKE_CONFIG", env_config.as_os_str());
    let _legacy_guard = EnvVarGuard::remove("NETSUKE_CONFIG_PATH");

    let merged = parse_and_merge(&["netsuke", "--config", &cli_config_arg])?;
    ensure!(
        merged.theme == Some(ThemePreference::Unicode),
        "--config should win over NETSUKE_CONFIG"
    );
    let _project_root = h.project.path();
    Ok(())
}

#[rstest]
fn config_flag_values_still_overridden_by_env_and_cli_preferences() -> Result<()> {
    let h = ConfigTestHarness::setup()?;
    let _legacy_guard = EnvVarGuard::remove("NETSUKE_CONFIG_PATH");
    let _theme_guard = EnvVarGuard::set("NETSUKE_THEME", "unicode");

    let custom_path = h.write_config("custom.toml", "theme = \"ascii\"\n")?;
    let custom_arg = custom_path.to_string_lossy().into_owned();

    let merged_with_cli_override =
        parse_and_merge(&["netsuke", "--config", &custom_arg, "--theme", "ascii"])?;
    ensure!(
        merged_with_cli_override.theme == Some(ThemePreference::Ascii),
        "CLI preference values should still override environment and selected config"
    );

    let merged_with_env_override = parse_and_merge(&["netsuke", "--config", &custom_arg])?;
    ensure!(
        merged_with_env_override.theme == Some(ThemePreference::Unicode),
        "environment preference values should still override the selected config"
    );
    let _project_root = h.project.path();
    Ok(())
}
