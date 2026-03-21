//! Configuration merge tests.
//!
//! These tests validate `OrthoConfig` layer precedence (defaults, file, env,
//! CLI) and list-value appending.

use anyhow::{Context, Result, ensure};
use netsuke::cli::Cli;
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

#[fixture]
fn default_cli_json() -> Result<serde_json::Value> {
    Ok(sanitize_value(&Cli::default())?)
}

#[rstest]
fn cli_merge_layers_respects_precedence_and_appends_lists(
    default_cli_json: Result<serde_json::Value>,
) -> Result<()> {
    let mut composer = MergeComposer::new();
    let mut defaults = default_cli_json?;
    let defaults_object = defaults
        .as_object_mut()
        .context("defaults should be an object")?;
    defaults_object.insert("jobs".to_owned(), json!(1));
    defaults_object.insert("fetch_allow_scheme".to_owned(), json!(["https"]));
    defaults_object.insert("progress".to_owned(), json!(true));
    defaults_object.insert("diag_json".to_owned(), json!(false));
    defaults_object.insert("theme".to_owned(), json!("auto"));
    composer.push_defaults(defaults);
    composer.push_file(
        json!({
            "file": "Configfile",
            "jobs": 2,
            "fetch_allow_scheme": ["http"],
            "locale": "en-US",
            "progress": false,
            "diag_json": true,
            "theme": "ascii"
        }),
        None,
    );
    composer.push_environment(json!({
        "jobs": 3,
        "fetch_allow_scheme": ["ftp"],
        "progress": true,
        "diag_json": false,
        "theme": "unicode"
    }));
    composer.push_cli(json!({
        "jobs": 4,
        "fetch_allow_scheme": ["git"],
        "progress": false,
        "diag_json": true,
        "theme": "ascii",
        "verbose": true
    }));
    let merged = Cli::merge_from_layers(composer.layers())?;
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
    ensure!(merged.verbose, "CLI layer should set verbose");
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
"#;
    fs::write(&config_path, config).context("write netsuke.toml")?;

    let _config_guard = EnvVarGuard::set("NETSUKE_CONFIG_PATH", config_path.as_os_str());
    let _jobs_guard = EnvVarGuard::set("NETSUKE_JOBS", OsStr::new("4"));
    let _theme_guard = EnvVarGuard::set("NETSUKE_THEME", OsStr::new("unicode"));
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
        merged.diag_json,
        "config diag_json should apply when CLI and env do not override",
    );

    Ok(())
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
