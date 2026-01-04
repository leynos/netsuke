//! Configuration merge tests.
//!
//! These tests validate OrthoConfig layer precedence (defaults, file, env,
//! CLI) and list-value appending.

use anyhow::{Context, Result, ensure};
use netsuke::cli::Cli;
use netsuke::cli_localization;
use ortho_config::{MergeComposer, sanitize_value};
use rstest::rstest;
use serde_json::json;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use tempfile::tempdir;
use test_support::{EnvVarGuard, env_lock::EnvLock};

#[rstest]
fn cli_merge_layers_respects_precedence_and_appends_lists() -> Result<()> {
    let mut composer = MergeComposer::new();
    let mut defaults = sanitize_value(&Cli::default())?;
    let defaults_object = defaults
        .as_object_mut()
        .context("defaults should be an object")?;
    defaults_object.insert("jobs".to_owned(), json!(1));
    defaults_object.insert("fetch_allow_scheme".to_owned(), json!(["https"]));
    composer.push_defaults(defaults);
    composer.push_file(
        json!({
            "file": "Configfile",
            "jobs": 2,
            "fetch_allow_scheme": ["http"],
            "locale": "en-US"
        }),
        None,
    );
    composer.push_environment(json!({
        "jobs": 3,
        "fetch_allow_scheme": ["ftp"]
    }));
    composer.push_cli(json!({
        "jobs": 4,
        "fetch_allow_scheme": ["git"],
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
"#;
    fs::write(&config_path, config).context("write netsuke.toml")?;

    let _config_guard = EnvVarGuard::set("NETSUKE_CONFIG_PATH", config_path.as_os_str());
    let _jobs_guard = EnvVarGuard::set("NETSUKE_JOBS", OsStr::new("4"));
    let _scheme_guard = EnvVarGuard::remove("NETSUKE_FETCH_ALLOW_SCHEME");

    let localizer = cli_localization::build_localizer(None);
    let (cli, matches) = netsuke::cli::parse_with_localizer_from(["netsuke"], localizer.as_ref())
        .context("parse CLI args for merge")?;
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

    Ok(())
}
