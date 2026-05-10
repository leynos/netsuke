//! Shared helpers for CLI tests.

use anyhow::{Context, Result, ensure};
use netsuke::cli::Cli;
use netsuke::cli::config::{ColourPolicy, OutputFormat, SpinnerMode};
use netsuke::theme::ThemePreference;
use ortho_config::MergeComposer;
use serde_json::json;
use std::ffi::OsString;
use std::path::Path;

pub(super) fn os_args(args: &[&str]) -> Vec<OsString> {
    args.iter().map(|arg| OsString::from(*arg)).collect()
}

/// RAII guard that restores the CWD on drop. Acquire after `EnvLock` so
/// the CWD is restored before the lock releases.
pub(super) struct CwdGuard(std::path::PathBuf);

impl CwdGuard {
    pub(super) fn acquire() -> anyhow::Result<Self> {
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

pub(super) fn build_precedence_and_append_composer(defaults: serde_json::Value) -> MergeComposer {
    let mut composer = MergeComposer::new();
    let mut seeded_defaults = defaults;
    let Some(defaults_object) = seeded_defaults.as_object_mut() else {
        panic!("defaults should be an object");
    };
    defaults_object.insert("jobs".to_owned(), json!(1));
    defaults_object.insert("fetch_allow_scheme".to_owned(), json!(["https"]));
    defaults_object.insert("progress".to_owned(), json!(true));
    defaults_object.insert("diag_json".to_owned(), json!(false));
    defaults_object.insert("theme".to_owned(), json!("auto"));
    defaults_object.insert("colour_policy".to_owned(), json!("auto"));
    defaults_object.insert("spinner_mode".to_owned(), json!("enabled"));
    defaults_object.insert("output_format".to_owned(), json!("human"));
    defaults_object.insert("default_targets".to_owned(), json!(["fmt"]));
    composer.push_defaults(seeded_defaults);
    composer.push_file(
        json!({
            "file": "Configfile",
            "jobs": 2,
            "fetch_allow_scheme": ["http"],
            "locale": "en-US",
            "progress": false,
            "diag_json": true,
            "theme": "ascii",
            "colour_policy": "never",
            "spinner_mode": "disabled",
            "output_format": "json",
            "default_targets": ["lint"]
        }),
        None,
    );
    composer.push_environment(json!({
        "jobs": 3,
        "fetch_allow_scheme": ["ftp"],
        "progress": true,
        "diag_json": false,
        "theme": "unicode",
        "colour_policy": "always",
        "spinner_mode": "enabled",
        "output_format": "human",
        "default_targets": ["test"]
    }));
    composer.push_cli(json!({
        "jobs": 4,
        "fetch_allow_scheme": ["git"],
        "progress": false,
        "diag_json": true,
        "theme": "ascii",
        "colour_policy": "never",
        "spinner_mode": "disabled",
        "output_format": "json",
        "default_targets": ["build"],
        "verbose": true
    }));
    composer
}

pub(super) fn assert_precedence_and_append_invariants(merged: &Cli) -> Result<()> {
    ensure!(
        merged.file.as_path() == Path::new("Configfile"),
        "file layer should override defaults",
    );
    ensure!(merged.jobs == Some(4), "CLI layer should override jobs");
    ensure!(
        merged.fetch_allow_scheme == vec!["https", "http", "ftp", "git"],
        "list values should append in layer order",
    );
    ensure!(
        merged.progress == Some(false),
        "CLI layer should override progress setting",
    );
    ensure!(
        merged.diag_json,
        "CLI layer should override diag_json setting",
    );
    ensure!(
        merged.locale.as_deref() == Some("en-US"),
        "file layer should populate locale when CLI does not override",
    );
    ensure!(
        merged.theme == Some(ThemePreference::Ascii),
        "CLI layer should override theme selection",
    );
    ensure!(
        merged.colour_policy == Some(ColourPolicy::Never),
        "CLI layer should override colour_policy",
    );
    ensure!(
        merged.spinner_mode == Some(SpinnerMode::Disabled),
        "CLI layer should override spinner_mode",
    );
    ensure!(
        merged.output_format == Some(OutputFormat::Json),
        "CLI layer should override output_format",
    );
    ensure!(
        merged.default_targets == vec!["fmt", "lint", "test", "build"],
        "default_targets should append in layer order",
    );
    ensure!(
        merged.resolved_diag_json(),
        "output_format=json should resolve to diagnostic JSON",
    );
    ensure!(
        !merged.resolved_progress(),
        "spinner_mode=disabled should resolve to no progress",
    );
    ensure!(merged.verbose, "CLI layer should set verbose");
    Ok(())
}

pub(super) fn assert_config_skips_empty_cli_layer_invariants(merged: &Cli) -> Result<()> {
    ensure!(
        merged.file.as_path() == Path::new("Configfile"),
        "config file should override the default manifest path",
    );
    ensure!(
        merged.verbose,
        "config file should override the default verbose flag",
    );
    ensure!(
        merged.fetch_default_deny,
        "config file should override the default deny flag",
    );
    ensure!(
        merged.jobs == Some(4),
        "environment variables should override config when CLI has no value",
    );
    ensure!(
        merged.fetch_allow_scheme == vec!["https".to_owned()],
        "config values should apply when CLI overrides are empty",
    );
    ensure!(
        merged.locale.as_deref() == Some("es-ES"),
        "config locale should be retained when CLI does not override",
    );
    ensure!(
        merged.progress == Some(false),
        "config progress should apply when CLI and env do not override",
    );
    ensure!(
        merged.theme == Some(ThemePreference::Unicode),
        "environment theme should override config when CLI has no value",
    );
    ensure!(
        merged.colour_policy == Some(ColourPolicy::Always),
        "environment colour_policy should override config",
    );
    ensure!(
        merged.spinner_mode == Some(SpinnerMode::Disabled),
        "config spinner_mode should apply when env does not override",
    );
    ensure!(
        merged.output_format == Some(OutputFormat::Json),
        "config output_format should apply when env does not override",
    );
    ensure!(
        merged.default_targets == vec![String::from("hello")],
        "config default_targets should be retained",
    );
    ensure!(
        merged.resolved_diag_json(),
        "config output_format should resolve to JSON diagnostics",
    );
    ensure!(
        !merged.resolved_progress(),
        "config spinner_mode should resolve to disabled progress",
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Unix config-environment fixture
// ---------------------------------------------------------------------------

/// Isolated test environment for Unix config-discovery tests.
///
/// Creates empty temporary directories for home and project, acquires
/// `EnvLock`, and sets `HOME`, `XDG_CONFIG_HOME`, and `XDG_CONFIG_DIRS`
/// to empty paths so host-level config files cannot leak into assertions.
#[cfg(unix)]
pub(super) struct UnixConfigTestEnv {
    _cwd_guard: CwdGuard,
    pub(super) temp_home: tempfile::TempDir,
    pub(super) temp_project: tempfile::TempDir,
    _xdg_home: tempfile::TempDir,
    _home_guard: test_support::EnvVarGuard,
    _xdg_home_guard: test_support::EnvVarGuard,
    _xdg_dirs_guard: test_support::EnvVarGuard,
    _config_path_guard: test_support::EnvVarGuard,
    _config_guard: test_support::EnvVarGuard,
    _diag_json_guard: test_support::EnvVarGuard,
    _output_format_guard: test_support::EnvVarGuard,
    pub(super) _env_lock: test_support::env_lock::EnvLock,
}

#[cfg(unix)]
#[rstest::fixture]
pub(super) fn unix_config_env() -> anyhow::Result<UnixConfigTestEnv> {
    use anyhow::Context;
    use std::ffi::OsStr;
    use tempfile::tempdir;
    use test_support::EnvVarGuard;
    use test_support::env_lock::EnvLock;

    let env_lock = EnvLock::acquire();
    let temp_home = tempdir().context("create temporary home directory")?;
    let temp_project = tempdir().context("create temporary project directory")?;
    let cwd_guard = CwdGuard::acquire().context("capture current working directory")?;
    let xdg_home = tempdir().context("create temporary XDG config home")?;
    let home_guard = EnvVarGuard::set("HOME", temp_home.path().as_os_str());
    let xdg_home_guard = EnvVarGuard::set("XDG_CONFIG_HOME", xdg_home.path().as_os_str());
    let xdg_dirs_guard = EnvVarGuard::set("XDG_CONFIG_DIRS", OsStr::new(""));
    let config_path_guard = EnvVarGuard::remove("NETSUKE_CONFIG_PATH");
    let config_guard = EnvVarGuard::remove("NETSUKE_CONFIG");
    let diag_json_guard = EnvVarGuard::remove("NETSUKE_DIAG_JSON");
    let output_format_guard = EnvVarGuard::remove("NETSUKE_OUTPUT_FORMAT");
    Ok(UnixConfigTestEnv {
        _cwd_guard: cwd_guard,
        temp_home,
        temp_project,
        _xdg_home: xdg_home,
        _home_guard: home_guard,
        _xdg_home_guard: xdg_home_guard,
        _xdg_dirs_guard: xdg_dirs_guard,
        _config_path_guard: config_path_guard,
        _config_guard: config_guard,
        _diag_json_guard: diag_json_guard,
        _output_format_guard: output_format_guard,
        _env_lock: env_lock,
    })
}
