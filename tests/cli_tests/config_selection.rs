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
//! Each test receives a [`ConfigTestHarness`] with isolated project and home
//! directories. Configuration merging runs in a child process whose
//! environment is assembled explicitly, so the harness process does not
//! mutate its environment or working directory. [`ConfigSelectionCase`] is a
//! const-buildable descriptor used by the main parametric test
//! [`config_selection_precedence_cases`].

use super::merge_probe::{isolated_environment, merge_in_child};
use anyhow::{Context, Result, ensure};
use netsuke::cli::EmojiPolicy;
use rstest::{fixture, rstest};
use std::ffi::OsString;
use std::fs;
use tempfile::tempdir;

struct ConfigTestHarness {
    home: tempfile::TempDir,
    project: tempfile::TempDir,
}

impl ConfigTestHarness {
    fn setup() -> Result<Self> {
        let project = tempdir().context("create project directory")?;
        let home = tempdir().context("create fake home directory")?;
        fs::create_dir_all(home.path().join(".config"))
            .context("create sandboxed XDG config home")?;
        Ok(Self { home, project })
    }

    fn write_config(&self, name: &str, content: &str) -> Result<std::path::PathBuf> {
        let path = self.project.path().join(name);
        fs::write(&path, content).with_context(|| format!("write config file {name}"))?;
        Ok(path)
    }

    fn merge(
        &self,
        args: &[&str],
        extra_environment: &[(OsString, OsString)],
    ) -> Result<netsuke::cli::Cli> {
        let (_xdg_config_dirs, environment) =
            isolated_environment(self.home.path(), extra_environment)?;
        merge_in_child(args, self.project.path(), &environment)
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
    let _project_config = write_optional_config(&h, case.project_config)?;
    let cli_config = write_optional_config(&h, case.cli_config)?;
    let env_config = write_optional_config(&h, case.env_config)?;

    let mut environment = Vec::new();
    if let Some(path) = env_config.as_deref() {
        environment.push((OsString::from("NETSUKE_CONFIG"), OsString::from(path)));
    }
    if let Some(emoji) = case.env_emoji {
        environment.push((OsString::from("NETSUKE_EMOJI"), OsString::from(emoji)));
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
    let merged = h.merge(&arg_refs, &environment)?;
    ensure!(merged.emoji == case.expected_emoji, "{}", case.message);
    Ok(())
}

#[rstest]
fn config_flag_with_nonexistent_file_produces_error(
    config_harness: Result<ConfigTestHarness>,
) -> Result<()> {
    let h = config_harness?;
    h.write_config(".netsuke.toml", "emoji = \"always\"\n")?;
    let error = match h.merge(&["netsuke", "--config", "missing.toml"], &[]) {
        Ok(value) => anyhow::bail!("missing explicit config file should fail: {value:?}"),
        Err(error) => error,
    };
    let message = format!("{error:?}");
    ensure!(
        message.contains("missing.toml"),
        "error should mention the missing explicit config path, got {message}"
    );
    // Pin the stable diagnostic alongside the path so a generic I/O failure
    // cannot satisfy this test.
    ensure!(
        message.contains("explicit configuration file not found"),
        "error should name the explicit-config failure mode, got {message}"
    );
    Ok(())
}
