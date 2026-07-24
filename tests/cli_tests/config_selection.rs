//! Integration tests for explicit configuration file selection and precedence.
//!
//! # Scope
//!
//! These tests exercise the user-visible configuration-selection contract
//! introduced in milestone 3.11.3:
//!
//! - `--config <PATH>` CLI flag (highest precedence)
//! - `NETSUKE_CONFIG` environment variable
//! - Automatic project-scope discovery (when no explicit selector is active)
//!
//! # Relationship to other test modules
//!
//! - [`config_discovery`](super::config_discovery): covers automatic
//!   multi-scope discovery and env-var overrides without an explicit
//!   `--config` flag; the two modules are complementary.
//! - [`merge`](super::merge): covers `OrthoConfig` layer-composition
//!   semantics (defaults → file → env → CLI); the present module targets
//!   the *selection* of which file enters that pipeline.
//! - `tests/features/configuration_discovery.feature`: BDD scenarios that
//!   duplicate the key precedence cases at the acceptance level.
//!
//! # Test infrastructure
//!
//! Every test receives a [`ConfigTestHarness`] via the `config_harness`
//! rstest fixture.  The harness:
//!
//! 1. Acquires [`EnvLock`] (serialises all env-mutating tests process-wide).
//! 2. Captures the process working directory via [`CwdGuard`].
//! 3. Creates isolated `project` and `home` tempdirs.
//! 4. Calls [`sandbox_user_scope`] to point `HOME`, `XDG_CONFIG_HOME`, and
//!    `XDG_CONFIG_DIRS` at the fake home so `OrthoConfig` user-scope discovery
//!    cannot read real host config files.
//! 5. Changes the process CWD to the project tempdir.
//!
//! Fields drop in declaration order; `_cwd_guard` is declared first so the
//! CWD is restored before `_env_lock` releases the mutex, preventing a race
//! where another test calls `std::env::current_dir()` while the CWD still
//! points at a deleted tempdir.
//!
//! [`parse_and_merge`] parses CLI arguments with a localiser and drives the
//! full `merge_with_config` pipeline, returning the merged [`Cli`] struct.
//! [`ConfigSelectionCase`] is a const-buildable descriptor used by the main
//! parametric test [`config_selection_precedence_cases`].

use anyhow::{Context, Result, ensure};
use netsuke::cli::EmojiPolicy;
use netsuke::cli_localization;
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
    env_emoji: Option<&'static str>,
    cli_emoji: Option<&'static str>,
    expected_emoji: EmojiPolicy,
    message: &'static str,
}

impl ConfigSelectionCase {
    const fn new(expected_emoji: EmojiPolicy, message: &'static str) -> Self {
        Self {
            project_config: None,
            cli_config: None,
            env_config: None,
            env_emoji: None,
            cli_emoji: None,
            expected_emoji,
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

    const fn with_env_emoji(mut self, emoji: &'static str) -> Self {
        self.env_emoji = Some(emoji);
        self
    }

    const fn with_cli_emoji(mut self, emoji: &'static str) -> Self {
        self.cli_emoji = Some(emoji);
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
        EmojiPolicy::Always,
        "explicit --config file should be loaded",
    )
    .with_cli_config(ConfigFile::new("custom.toml", "emoji = \"always\"\n")),
)]
#[case::config_flag_skips_project_discovery(
    ConfigSelectionCase::new(
        EmojiPolicy::Always,
        "explicit --config should bypass discovered project config",
    )
    .with_project_config(ConfigFile::new(".netsuke.toml", "emoji = \"never\"\n"))
    .with_cli_config(ConfigFile::new("custom.toml", "emoji = \"always\"\n")),
)]
#[case::netsuke_config_env_loads_specified_file(
    ConfigSelectionCase::new(
        EmojiPolicy::Always,
        "NETSUKE_CONFIG should load the selected config file",
    )
    .with_env_config(ConfigFile::new("env.toml", "emoji = \"always\"\n")),
)]
#[case::config_flag_takes_precedence_over_netsuke_config_env(
    ConfigSelectionCase::new(
        EmojiPolicy::Always,
        "--config should win over NETSUKE_CONFIG",
    )
    .with_cli_config(ConfigFile::new("cli.toml", "emoji = \"always\"\n"))
    .with_env_config(ConfigFile::new("env.toml", "emoji = \"never\"\n")),
)]
#[case::config_flag_values_still_overridden_by_cli_preferences(
    ConfigSelectionCase::new(
        EmojiPolicy::Never,
        "CLI preference values should still override environment and selected config",
    )
    .with_cli_config(ConfigFile::new("custom.toml", "emoji = \"never\"\n"))
    .with_env_emoji("always")
    .with_cli_emoji("never"),
)]
#[case::config_flag_values_still_overridden_by_env_preferences(
    ConfigSelectionCase::new(
        EmojiPolicy::Always,
        "environment preference values should still override the selected config",
    )
    .with_cli_config(ConfigFile::new("custom.toml", "emoji = \"never\"\n"))
    .with_env_emoji("always"),
)]
fn config_selection_precedence_cases(
    config_harness: Result<ConfigTestHarness>,
    #[case] case: ConfigSelectionCase,
) -> Result<()> {
    let h = config_harness?;
    let _config_guard = EnvVarGuard::remove("NETSUKE_CONFIG");
    let _emoji_guard = EnvVarGuard::remove("NETSUKE_EMOJI");

    let _project_config = write_optional_config(&h, case.project_config)?;
    let cli_config = write_optional_config(&h, case.cli_config)?;
    let env_config = write_optional_config(&h, case.env_config)?;

    let mut env_guards = Vec::new();
    if let Some(path) = env_config.as_deref() {
        env_guards.push(EnvVarGuard::set("NETSUKE_CONFIG", path));
    }
    if let Some(emoji) = case.env_emoji {
        env_guards.push(EnvVarGuard::set("NETSUKE_EMOJI", emoji));
    }

    let mut args = vec![String::from("netsuke")];
    if let Some(path) = cli_config {
        args.push(String::from("--config"));
        args.push(path);
    }
    if let Some(emoji) = case.cli_emoji {
        args.push(String::from("--emoji"));
        args.push(String::from(emoji));
    }

    let arg_refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    let merged = parse_and_merge(&arg_refs)?;
    ensure!(merged.emoji == case.expected_emoji, "{}", case.message);
    let _project_root = h.project.path();
    drop(env_guards);
    Ok(())
}

#[rstest]
fn config_flag_with_nonexistent_file_produces_error(
    config_harness: Result<ConfigTestHarness>,
) -> Result<()> {
    let h = config_harness?;
    h.write_config(".netsuke.toml", "emoji = \"always\"\n")?;
    let _config_guard = EnvVarGuard::remove("NETSUKE_CONFIG");

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
