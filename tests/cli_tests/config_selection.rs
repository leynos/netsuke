//! Integration tests for explicit configuration file selection.
//!
//! These tests cover the visible `--config` flag and `NETSUKE_CONFIG`
//! environment variable, plus compatibility with the legacy
//! `NETSUKE_CONFIG_PATH` override.

use anyhow::{Context, Result, ensure};
use netsuke::cli_localization;
use netsuke::theme::ThemePreference;
use rstest::{fixture, rstest};
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

#[fixture]
fn config_harness() -> Result<ConfigTestHarness> {
    ConfigTestHarness::setup()
}

#[derive(Clone, Copy)]
struct ConfigFile {
    name: &'static str,
    content: &'static str,
}

impl ConfigFile {
    const fn new(name: &'static str, content: &'static str) -> Self {
        Self { name, content }
    }
}

#[derive(Clone, Copy)]
struct ConfigSelectionCase {
    project_config: Option<ConfigFile>,
    cli_config: Option<ConfigFile>,
    env_config: Option<ConfigFile>,
    legacy_config: Option<ConfigFile>,
    env_theme: Option<&'static str>,
    cli_theme: Option<&'static str>,
    expected_theme: ThemePreference,
    message: &'static str,
}

impl ConfigSelectionCase {
    const fn new(expected_theme: ThemePreference, message: &'static str) -> Self {
        Self {
            project_config: None,
            cli_config: None,
            env_config: None,
            legacy_config: None,
            env_theme: None,
            cli_theme: None,
            expected_theme,
            message,
        }
    }

    const fn with_project_config(mut self, config: ConfigFile) -> Self {
        self.project_config = Some(config);
        self
    }

    const fn with_cli_config(mut self, config: ConfigFile) -> Self {
        self.cli_config = Some(config);
        self
    }

    const fn with_env_config(mut self, config: ConfigFile) -> Self {
        self.env_config = Some(config);
        self
    }

    const fn with_legacy_config(mut self, config: ConfigFile) -> Self {
        self.legacy_config = Some(config);
        self
    }

    const fn with_env_theme(mut self, theme: &'static str) -> Self {
        self.env_theme = Some(theme);
        self
    }

    const fn with_cli_theme(mut self, theme: &'static str) -> Self {
        self.cli_theme = Some(theme);
        self
    }
}

fn write_optional_config(
    harness: &ConfigTestHarness,
    config: Option<ConfigFile>,
) -> Result<Option<String>> {
    config
        .map(|file| {
            harness
                .write_config(file.name, file.content)
                .map(|path| path.to_string_lossy().into_owned())
        })
        .transpose()
}

#[rstest]
#[case::config_flag_loads_specified_file(
    ConfigSelectionCase::new(
        ThemePreference::Unicode,
        "explicit --config file should be loaded",
    )
    .with_cli_config(ConfigFile::new("custom.toml", "theme = \"unicode\"\n")),
)]
#[case::config_flag_skips_project_discovery(
    ConfigSelectionCase::new(
        ThemePreference::Unicode,
        "explicit --config should bypass discovered project config",
    )
    .with_project_config(ConfigFile::new(".netsuke.toml", "theme = \"ascii\"\n"))
    .with_cli_config(ConfigFile::new("custom.toml", "theme = \"unicode\"\n")),
)]
#[case::netsuke_config_env_loads_specified_file(
    ConfigSelectionCase::new(
        ThemePreference::Unicode,
        "NETSUKE_CONFIG should load the selected config file",
    )
    .with_env_config(ConfigFile::new("env.toml", "theme = \"unicode\"\n")),
)]
#[case::netsuke_config_env_takes_precedence_over_legacy(
    ConfigSelectionCase::new(
        ThemePreference::Unicode,
        "NETSUKE_CONFIG should win over NETSUKE_CONFIG_PATH",
    )
    .with_env_config(ConfigFile::new("new.toml", "theme = \"unicode\"\n"))
    .with_legacy_config(ConfigFile::new("legacy.toml", "theme = \"ascii\"\n")),
)]
#[case::config_flag_takes_precedence_over_netsuke_config_env(
    ConfigSelectionCase::new(
        ThemePreference::Unicode,
        "--config should win over NETSUKE_CONFIG",
    )
    .with_cli_config(ConfigFile::new("cli.toml", "theme = \"unicode\"\n"))
    .with_env_config(ConfigFile::new("env.toml", "theme = \"ascii\"\n")),
)]
#[case::config_flag_values_still_overridden_by_cli_preferences(
    ConfigSelectionCase::new(
        ThemePreference::Ascii,
        "CLI preference values should still override environment and selected config",
    )
    .with_cli_config(ConfigFile::new("custom.toml", "theme = \"ascii\"\n"))
    .with_env_theme("unicode")
    .with_cli_theme("ascii"),
)]
#[case::config_flag_values_still_overridden_by_env_preferences(
    ConfigSelectionCase::new(
        ThemePreference::Unicode,
        "environment preference values should still override the selected config",
    )
    .with_cli_config(ConfigFile::new("custom.toml", "theme = \"ascii\"\n"))
    .with_env_theme("unicode"),
)]
fn config_selection_precedence_cases(
    config_harness: Result<ConfigTestHarness>,
    #[case] case: ConfigSelectionCase,
) -> Result<()> {
    let h = config_harness?;
    let _config_guard = EnvVarGuard::remove("NETSUKE_CONFIG");
    let _legacy_guard = EnvVarGuard::remove("NETSUKE_CONFIG_PATH");
    let _theme_guard = EnvVarGuard::remove("NETSUKE_THEME");

    let _project_config = write_optional_config(&h, case.project_config)?;
    let cli_config = write_optional_config(&h, case.cli_config)?;
    let env_config = write_optional_config(&h, case.env_config)?;
    let legacy_config = write_optional_config(&h, case.legacy_config)?;

    let mut env_guards = Vec::new();
    if let Some(path) = env_config.as_deref() {
        env_guards.push(EnvVarGuard::set("NETSUKE_CONFIG", path));
    }
    if let Some(path) = legacy_config.as_deref() {
        env_guards.push(EnvVarGuard::set("NETSUKE_CONFIG_PATH", path));
    }
    if let Some(theme) = case.env_theme {
        env_guards.push(EnvVarGuard::set("NETSUKE_THEME", theme));
    }

    let mut args = vec![String::from("netsuke")];
    if let Some(path) = cli_config {
        args.push(String::from("--config"));
        args.push(path);
    }
    if let Some(theme) = case.cli_theme {
        args.push(String::from("--theme"));
        args.push(String::from(theme));
    }

    let arg_refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    let merged = parse_and_merge(&arg_refs)?;
    ensure!(
        merged.theme == Some(case.expected_theme),
        "{}",
        case.message
    );
    let _project_root = h.project.path();
    drop(env_guards);
    Ok(())
}

#[rstest]
fn config_flag_with_nonexistent_file_produces_error(
    config_harness: Result<ConfigTestHarness>,
) -> Result<()> {
    let h = config_harness?;
    h.write_config(".netsuke.toml", "theme = \"unicode\"\n")?;
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
