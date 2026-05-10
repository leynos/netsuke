//! Configuration merge tests.
//!
//! These tests validate `OrthoConfig` layer precedence (defaults, file, env,
//! CLI) and list-value appending.

#[cfg(unix)]
use super::helpers::unix_config_env;
use super::helpers::{
    CwdGuard, assert_config_skips_empty_cli_layer_invariants,
    assert_precedence_and_append_invariants, build_precedence_and_append_composer,
};
use anyhow::{Context, Result, ensure};
use netsuke::cli::Cli;
use netsuke::cli_localization;
use netsuke::theme::ThemePreference;
use ortho_config::{MergeComposer, sanitize_value};
use rstest::{fixture, rstest};
use serde_json::json;
use std::ffi::OsStr;
use std::fs;
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
    let composer = build_precedence_and_append_composer(default_cli_json?);
    let merged = Cli::merge_from_layers(composer.layers())?;
    assert_precedence_and_append_invariants(&merged)
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
    let _diag_json_guard = EnvVarGuard::remove("NETSUKE_DIAG_JSON");
    let _output_format_guard = EnvVarGuard::remove("NETSUKE_OUTPUT_FORMAT");
    let _netsuke_config_guard = EnvVarGuard::remove("NETSUKE_CONFIG");

    let localizer = Arc::from(cli_localization::build_localizer(None));
    let (cli, matches) = netsuke::cli::parse_with_localizer_from(["netsuke"], &localizer)
        .context("parse CLI args for merge")?;
    ensure!(
        netsuke::cli::resolve_merged_diag_json(&cli, &matches)?,
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
    let _diag_json_guard = EnvVarGuard::remove("NETSUKE_DIAG_JSON");
    let _output_format_guard = EnvVarGuard::remove("NETSUKE_OUTPUT_FORMAT");
    let _netsuke_config_guard = EnvVarGuard::remove("NETSUKE_CONFIG");

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
fn resolve_merged_diag_json_handles_malformed_project_config(
    unix_config_env: Result<super::helpers::UnixConfigTestEnv>,
) -> Result<()> {
    let env = unix_config_env?;

    // User config: valid, sets output_format=json
    let user_config = env.temp_home.path().join(".netsuke.toml");
    fs::write(&user_config, "output_format = \"json\"\n").context("write user .netsuke.toml")?;

    // Project config: malformed (missing closing quote)
    let project_config = env.temp_project.path().join(".netsuke.toml");
    fs::write(&project_config, "theme = \"ascii\n")
        .context("write malformed project .netsuke.toml")?;

    let _env_lock = EnvLock::acquire();
    let _cwd_guard = CwdGuard::acquire()?;
    std::env::set_current_dir(&env.temp_project).context("change to project directory")?;

    let localizer = Arc::from(cli_localization::build_localizer(None));
    let (cli, matches) = netsuke::cli::parse_with_localizer_from(["netsuke"], &localizer)
        .context("parse CLI for malformed project config test")?;

    let error = netsuke::cli::resolve_merged_diag_json(&cli, &matches)
        .expect_err("malformed project config should surface before merge");
    ensure!(
        format!("{error:?}").contains(".netsuke.toml"),
        "error should mention the malformed project config"
    );

    let merge_error = netsuke::cli::merge_with_config(&cli, &matches)
        .expect_err("merge_with_config should fail for malformed project config");
    ensure!(
        format!("{merge_error:?}").contains(".netsuke.toml"),
        "merge error should mention the malformed project config"
    );

    Ok(())
}

#[cfg(unix)]
#[rstest]
fn resolve_merged_diag_json_does_not_discover_after_explicit_config_error(
    unix_config_env: Result<super::helpers::UnixConfigTestEnv>,
) -> Result<()> {
    let env = unix_config_env?;

    fs::write(
        env.temp_project.path().join(".netsuke.toml"),
        "output_format = \"json\"\n",
    )
    .context("write project .netsuke.toml")?;

    let explicit_config = env.temp_project.path().join("broken.toml");
    fs::write(&explicit_config, "theme = \"ascii\n").context("write malformed explicit config")?;

    let _env_lock = EnvLock::acquire();
    let _cwd_guard = CwdGuard::acquire()?;
    std::env::set_current_dir(&env.temp_project).context("change to project directory")?;

    let config_arg = explicit_config.to_string_lossy().into_owned();
    let localizer = Arc::from(cli_localization::build_localizer(None));
    let (cli, matches) =
        netsuke::cli::parse_with_localizer_from(["netsuke", "--config", &config_arg], &localizer)
            .context("parse CLI with malformed explicit config")?;

    let error = netsuke::cli::resolve_merged_diag_json(&cli, &matches)
        .expect_err("malformed explicit config should surface before discovery fallback");
    ensure!(
        format!("{error:?}").contains("broken.toml"),
        "error should mention the malformed explicit config"
    );

    let (cli_with_diag, matches_with_diag) = netsuke::cli::parse_with_localizer_from(
        ["netsuke", "--config", &config_arg, "--diag-json"],
        &localizer,
    )
    .context("parse CLI with explicit diagnostic JSON flag")?;
    let diag_flag_error =
        netsuke::cli::resolve_merged_diag_json(&cli_with_diag, &matches_with_diag)
            .expect_err("malformed explicit config should surface even with --diag-json");
    ensure!(
        format!("{diag_flag_error:?}").contains("broken.toml"),
        "error should mention the malformed explicit config"
    );

    Ok(())
}
