//! Configuration merge tests.
//!
//! These tests validate OrthoConfig layer precedence (defaults, file, env,
//! CLI) and list-value appending.

use anyhow::{Context, Result, ensure};
use netsuke::cli::{CliConfig, Theme};
use netsuke::cli_localization;
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
    sanitize_value(&CliConfig::default())
}

fn with_config_file<F, T>(toml_content: &str, cli_args: &[&str], f: F) -> anyhow::Result<T>
where
    F: FnOnce(netsuke::cli::Cli) -> anyhow::Result<T>,
{
    let _env_lock = test_support::env_lock::EnvLock::acquire();
    let temp_dir = tempfile::tempdir().context("create temporary config directory")?;
    let config_path = temp_dir.path().join("netsuke.toml");
    std::fs::write(&config_path, toml_content).context("write netsuke.toml")?;
    let _config_guard =
        test_support::EnvVarGuard::set("NETSUKE_CONFIG_PATH", config_path.as_os_str());
    let localizer = std::sync::Arc::from(netsuke::cli_localization::build_localizer(None));
    let (cli, matches) = netsuke::cli::parse_with_localizer_from(cli_args, &localizer)
        .context("parse CLI args for merge")?;
    let merged = netsuke::cli::merge_with_config(&cli, &matches)
        .context("merge CLI and configuration layers")?;
    f(merged)
}

fn assert_build_targets(
    toml_content: &str,
    cli_args: &[&str],
    expected_targets: &[String],
    assertion_msg: &str,
) -> anyhow::Result<()> {
    with_config_file(toml_content, cli_args, |merged| {
        let Some(netsuke::cli::Commands::Build(args)) = merged.command else {
            anyhow::bail!("expected merged command to be build");
        };
        ensure!(args.targets == expected_targets, "{}", assertion_msg);
        Ok(())
    })
}

fn merge_defaults_with_file_layer(
    defaults: serde_json::Value,
    file_layer: serde_json::Value,
) -> anyhow::Result<netsuke::cli::CliConfig> {
    let mut composer = ortho_config::MergeComposer::new();
    composer.push_defaults(defaults);
    composer.push_file(file_layer, None);
    netsuke::cli::CliConfig::merge_from_layers(composer.layers()).map_err(anyhow::Error::from)
}

fn assert_merge_rejects(
    defaults: serde_json::Value,
    file_layer: serde_json::Value,
    expect_err_hint: &str,
    expected_msg: &str,
) -> anyhow::Result<()> {
    let err = merge_defaults_with_file_layer(defaults, file_layer).expect_err(expect_err_hint);
    ensure!(
        err.to_string().contains(expected_msg),
        "unexpected error text: {err}",
    );
    Ok(())
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
    composer.push_defaults(defaults);
    composer.push_file(
        json!({
            "file": "Configfile",
            "jobs": 2,
            "fetch_allow_scheme": ["http"],
            "locale": "en-US",
            "progress": false,
            "diag_json": true
        }),
        None,
    );
    composer.push_environment(json!({
        "jobs": 3,
        "fetch_allow_scheme": ["ftp"],
        "progress": true,
        "diag_json": false
    }));
    composer.push_cli(json!({
        "jobs": 4,
        "fetch_allow_scheme": ["git"],
        "progress": false,
        "diag_json": true,
        "verbose": true
    }));
    let merged = CliConfig::merge_from_layers(composer.layers())?;
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
"#;
    fs::write(&config_path, config).context("write netsuke.toml")?;

    let _config_guard = EnvVarGuard::set("NETSUKE_CONFIG_PATH", config_path.as_os_str());
    let _jobs_guard = EnvVarGuard::set("NETSUKE_JOBS", OsStr::new("4"));
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
        merged.diag_json,
        "config diag_json should apply when CLI and env do not override",
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

    let merged = CliConfig::merge_from_layers(composer.layers())?;
    ensure!(
        merged.locale.as_deref() == Some("en-US"),
        "CLI locale should override env and file layers",
    );
    Ok(())
}

#[rstest]
fn cli_config_build_defaults_apply_when_cli_targets_are_absent() -> Result<()> {
    assert_build_targets(
        r#"
[cmds.build]
targets = ["all", "docs"]
"#,
        &["netsuke"],
        &[String::from("all"), String::from("docs")],
        "configured build targets should be used when CLI targets are absent",
    )
}

#[rstest]
fn cli_config_explicit_targets_override_configured_build_defaults() -> Result<()> {
    assert_build_targets(
        r#"
[cmds.build]
targets = ["all"]
"#,
        &["netsuke", "build", "lint"],
        &[String::from("lint")],
        "explicit CLI targets should override configured defaults",
    )
}

#[rstest]
fn cli_config_validates_theme_alias_conflicts(
    default_cli_json: Result<serde_json::Value>,
) -> Result<()> {
    assert_merge_rejects(
        default_cli_json?,
        json!({
            "theme": "unicode",
            "no_emoji": true
        }),
        "conflicting theme and alias should fail",
        "theme = \"unicode\" conflicts with no_emoji = true",
    )
}

#[rstest]
fn cli_config_validates_spinner_and_progress_conflicts(
    default_cli_json: Result<serde_json::Value>,
) -> Result<()> {
    assert_merge_rejects(
        default_cli_json?,
        json!({
            "spinner_mode": "disabled",
            "progress": true
        }),
        "conflicting spinner and progress settings should fail",
        "spinner_mode = \"disabled\" conflicts with progress = true",
    )
}

#[rstest]
fn cli_config_rejects_unsupported_json_output_format(
    default_cli_json: Result<serde_json::Value>,
) -> Result<()> {
    let err = merge_defaults_with_file_layer(
        default_cli_json?,
        json!({
            "output_format": "json"
        }),
    )
    .expect_err("unsupported output format should fail");
    ensure!(
        err.to_string().contains("output_format = \"json\" is not supported yet"),
        "unexpected error text: {err}",
    );
    Ok(())
}

#[rstest]
fn cli_runtime_canonicalizes_ascii_theme_from_no_emoji_alias() -> Result<()> {
    with_config_file("no_emoji = true\n", &["netsuke"], |merged| {
        ensure!(
            merged.theme == Some(Theme::Ascii),
            "no_emoji compatibility alias should canonicalize to the ASCII theme",
        );
        ensure!(
            merged.no_emoji == Some(true),
            "no_emoji alias should remain available in the runtime CLI",
        );
        Ok(())
    })
}
