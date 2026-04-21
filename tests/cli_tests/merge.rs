//! Configuration merge tests.
//!
//! These tests validate `OrthoConfig` layer precedence (defaults, file, env,
//! CLI) and list-value appending.

use anyhow::{Context, Result, ensure};
use netsuke::cli::Cli;
use netsuke::cli::config::{ColourPolicy, OutputFormat, SpinnerMode};
use netsuke::cli_localization;
use netsuke::theme::ThemePreference;
use ortho_config::{MergeComposer, sanitize_value};
use rstest::{fixture, rstest};
use serde_json::json;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tempfile::tempdir;
use test_support::{EnvVarGuard, env_lock::EnvLock};

/// RAII guard that restores the process working directory on drop.
///
/// Acquire this *after* `EnvLock` so the drop order (CWD restored first,
/// lock released second) mirrors the acquire order.
struct CwdGuard(std::path::PathBuf);

impl CwdGuard {
    fn acquire() -> anyhow::Result<Self> {
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

#[fixture]
fn default_cli_json() -> Result<serde_json::Value> {
    Ok(sanitize_value(&Cli::default())?)
}

fn build_precedence_and_append_composer(defaults: serde_json::Value) -> MergeComposer {
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

fn assert_precedence_and_append_invariants(merged: &Cli) -> Result<()> {
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

#[rstest]
fn cli_merge_layers_respects_precedence_and_appends_lists(
    default_cli_json: Result<serde_json::Value>,
) -> Result<()> {
    let composer = build_precedence_and_append_composer(default_cli_json?);
    let merged = Cli::merge_from_layers(composer.layers())?;
    assert_precedence_and_append_invariants(&merged)
}

fn assert_config_skips_empty_cli_layer_invariants(merged: &Cli) -> Result<()> {
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

#[rstest]
fn cli_merge_with_config_respects_precedence_and_skips_empty_cli_layer() -> Result<()> {
    let _env_lock = EnvLock::acquire();
    let temp_dir = tempdir().context("create temporary config directory")?;
    let config_path = temp_dir.path().join("netsuke.toml");
    let config = r#"
file = "Configfile"
jobs = 2
fetch_allow_scheme = ["https"]
verbose = true
fetch_default_deny = true
locale = "es-ES"
progress = false
diag_json = true
theme = "ascii"
colour_policy = "never"
spinner_mode = "disabled"
output_format = "json"
default_targets = ["hello"]
"#;
    fs::write(&config_path, config).context("write netsuke.toml")?;

    let _config_guard = EnvVarGuard::set("NETSUKE_CONFIG_PATH", config_path.as_os_str());
    let _jobs_guard = EnvVarGuard::set("NETSUKE_JOBS", OsStr::new("4"));
    let _theme_guard = EnvVarGuard::set("NETSUKE_THEME", OsStr::new("unicode"));
    let _colour_policy_guard = EnvVarGuard::set("NETSUKE_COLOUR_POLICY", OsStr::new("always"));
    let _scheme_guard = EnvVarGuard::remove("NETSUKE_FETCH_ALLOW_SCHEME");

    let localizer = Arc::from(cli_localization::build_localizer(None));
    let (cli, matches) = netsuke::cli::parse_with_localizer_from(["netsuke"], &localizer)
        .context("parse CLI args for merge")?;
    ensure!(
        netsuke::cli::resolve_merged_diag_json(&cli, &matches),
        "pre-merge diagnostic mode should honour config diag_json",
    );
    let merged = netsuke::cli::merge_with_config(&cli, &matches)
        .context("merge CLI and configuration layers")?
        .with_default_command();
    assert_config_skips_empty_cli_layer_invariants(&merged)
}

#[rstest]
fn cli_merge_with_config_prefers_cli_theme_over_env_and_file() -> Result<()> {
    let _env_lock = EnvLock::acquire();
    let temp_dir = tempdir().context("create temporary config directory")?;
    let config_path = temp_dir.path().join("netsuke.toml");
    fs::write(&config_path, "theme = \"ascii\"\n").context("write netsuke.toml")?;

    let _config_guard = EnvVarGuard::set("NETSUKE_CONFIG_PATH", config_path.as_os_str());
    let _theme_guard = EnvVarGuard::set("NETSUKE_THEME", OsStr::new("unicode"));

    let localizer = Arc::from(cli_localization::build_localizer(None));
    let (cli, matches) =
        netsuke::cli::parse_with_localizer_from(["netsuke", "--theme", "ascii"], &localizer)
            .context("parse CLI args for theme override merge")?;
    let merged = netsuke::cli::merge_with_config(&cli, &matches)
        .context("merge theme across CLI, env, and config layers")?
        .with_default_command();

    ensure!(
        merged.theme == Some(ThemePreference::Ascii),
        "CLI theme should override env and config layers",
    );
    Ok(())
}

#[rstest]
fn cli_merge_layers_prefers_cli_then_env_then_file_for_locale(
    default_cli_json: Result<serde_json::Value>,
) -> Result<()> {
    let mut composer = MergeComposer::new();
    let defaults = default_cli_json?;
    composer.push_defaults(defaults);
    composer.push_file(json!({ "locale": "fr-FR" }), None);
    composer.push_environment(json!({ "locale": "es-ES" }));
    composer.push_cli(json!({ "locale": "en-US" }));

    let merged = Cli::merge_from_layers(composer.layers())?;
    ensure!(
        merged.locale.as_deref() == Some("en-US"),
        "CLI locale should override env and file layers",
    );
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn resolve_merged_diag_json_handles_malformed_project_config() -> Result<()> {
    let _env_lock = EnvLock::acquire();
    let cwd_guard = CwdGuard::acquire().context("capture current working directory")?;
    let temp_home = tempdir().context("create temporary home directory")?;
    let temp_project = tempdir().context("create temporary project directory")?;

    // User config: valid, sets output_format=json
    let user_config = temp_home.path().join(".netsuke.toml");
    fs::write(
        &user_config,
        r#"
output_format = "json"
"#,
    )
    .context("write user .netsuke.toml")?;

    // Project config: malformed (missing closing quote)
    let project_config = temp_project.path().join(".netsuke.toml");
    fs::write(
        &project_config,
        r#"
theme = "ascii
"#,
    )
    .context("write malformed project .netsuke.toml")?;

    let temp_xdg_home = tempdir().context("create temporary XDG config home")?;
    let _home_guard = EnvVarGuard::set("HOME", temp_home.path().as_os_str());
    let _xdg_home_guard = EnvVarGuard::set("XDG_CONFIG_HOME", temp_xdg_home.path().as_os_str());
    let _xdg_dirs_guard = EnvVarGuard::set("XDG_CONFIG_DIRS", OsStr::new(""));
    let _config_path_guard = EnvVarGuard::remove("NETSUKE_CONFIG_PATH");

    std::env::set_current_dir(&temp_project).context("change to project directory")?;

    let localizer = Arc::from(cli_localization::build_localizer(None));
    let (cli, matches) = netsuke::cli::parse_with_localizer_from(["netsuke"], &localizer)
        .context("parse CLI for malformed project config test")?;

    // resolve_merged_diag_json should not propagate the project config parse error,
    // but should fall back to the valid user config setting (output_format=json)
    ensure!(
        netsuke::cli::resolve_merged_diag_json(&cli, &matches),
        "should honour user config output_format=json despite malformed project config"
    );

    drop(cwd_guard);
    Ok(())
}
