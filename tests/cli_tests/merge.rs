//! Configuration merge tests.
//!
//! These tests validate `OrthoConfig` layer precedence (defaults, file, env,
//! CLI) and list-value appending.
use super::merge_probe::{isolated_environment, merge_in_child};
use anyhow::{Context, Result, ensure};
use netsuke::cli::{CliConfig, ProgressPolicy};
use ortho_config::{MergeComposer, sanitize_value};
use rstest::{fixture, rstest};
use serde_json::json;
use std::ffi::OsString;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

#[fixture]
fn default_cli_json() -> Result<serde_json::Value> {
    Ok(sanitize_value(&CliConfig::default())?)
}

fn with_config_file<F, T>(toml_content: &str, cli_args: &[&str], f: F) -> anyhow::Result<T>
where
    F: FnOnce(netsuke::cli::Cli) -> anyhow::Result<T>,
{
    let temp_dir = tempfile::tempdir().context("create temporary config directory")?;
    let config_path = temp_dir.path().join("netsuke.toml");
    std::fs::write(&config_path, toml_content).context("write netsuke.toml")?;
    // Seed the configuration sandbox so host XDG configuration cannot leak into
    // the child; `_xdg_config_dirs` must outlive the child process.
    let (_xdg_config_dirs, environment) = isolated_environment(
        temp_dir.path(),
        &[(
            OsString::from("NETSUKE_CONFIG"),
            config_path.into_os_string(),
        )],
    )?;
    let merged = merge_in_child(cli_args, temp_dir.path(), &environment)?;
    f(merged)
}

fn assert_build_targets(
    toml_content: &str,
    cli_args: &[&str],
    expected_targets: &[String],
) -> anyhow::Result<()> {
    with_config_file(toml_content, cli_args, |merged| {
        let Some(netsuke::cli::Commands::Build(args)) = merged.command else {
            anyhow::bail!("expected merged command to be build");
        };
        ensure!(
            args.targets == expected_targets,
            "build targets mismatch: got {:?}, expected {:?}",
            args.targets,
            expected_targets,
        );
        Ok(())
    })
}

#[derive(Debug, Copy, Clone)]
enum ExpectedValidationError {
    InteractiveInput,
    JobsOutOfRange,
}

impl ExpectedValidationError {
    const fn expected_fragment(self) -> &'static str {
        match self {
            Self::InteractiveInput => "no_input = false is unsupported",
            Self::JobsOutOfRange => "jobs = 65 is out of range",
        }
    }
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
    expected_error: ExpectedValidationError,
) -> anyhow::Result<()> {
    let err = match merge_defaults_with_file_layer(defaults, file_layer) {
        Ok(value) => anyhow::bail!("merge should have returned an error; got {value:#?}"),
        Err(err) => err,
    };
    ensure!(
        err.chain().any(|cause| cause
            .to_string()
            .contains(expected_error.expected_fragment())),
        "unexpected error text: {err:#}",
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
    defaults_object.insert("progress".to_owned(), json!("auto"));
    defaults_object.insert("json".to_owned(), json!(false));
    composer.push_defaults(defaults);
    composer.push_file(
        json!({
            "file": "Configfile",
            "jobs": 2,
            "fetch_allow_scheme": ["http"],
            "locale": "en-US",
            "progress": "never",
            "json": true
        }),
        None,
    );
    composer.push_environment(json!({
        "jobs": 3,
        "fetch_allow_scheme": ["ftp"],
        "progress": "always",
        "json": false
    }));
    composer.push_cli(json!({
        "jobs": 4,
        "fetch_allow_scheme": ["git"],
        "progress": "never",
        "json": true,
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
        merged.progress == ProgressPolicy::Never,
        "CLI layer should override progress setting",
    );
    ensure!(merged.json, "CLI layer should override json setting",);
    ensure!(
        merged.locale.as_deref() == Some("en-US"),
        "file layer should populate locale when CLI does not override",
    );
    ensure!(merged.verbose, "CLI layer should set verbose");
    Ok(())
}

#[rstest]
fn cli_merge_with_config_respects_precedence_and_skips_empty_cli_layer() -> Result<()> {
    let temp_dir = tempdir().context("create temporary config directory")?;
    let config_path = temp_dir.path().join("netsuke.toml");
    let config = r#"
file = "Configfile"
jobs = 2
fetch_allow_scheme = ["https"]
verbose = true
fetch_default_deny = true
locale = "es-ES"
progress = "never"
json = true
"#;
    fs::write(&config_path, config).context("write netsuke.toml")?;

    // As in `with_config_file`, sandbox the child's configuration lookup so the
    // precedence assertions cannot be perturbed by host XDG configuration.
    let (_xdg_config_dirs, environment) = isolated_environment(
        temp_dir.path(),
        &[
            (
                OsString::from("NETSUKE_CONFIG"),
                config_path.into_os_string(),
            ),
            (OsString::from("NETSUKE_JOBS"), OsString::from("4")),
        ],
    )?;
    let merged = merge_in_child(&["netsuke"], temp_dir.path(), &environment)?;
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
        merged.progress == ProgressPolicy::Never,
        "config progress should apply when CLI and env do not override",
    );
    ensure!(
        merged.json,
        "config json should apply when CLI and env do not override",
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
    )
}

#[rstest]
fn cli_default_target_is_preserved_for_build() -> Result<()> {
    with_config_file(
        "",
        &["netsuke", "--default-target", "all", "build"],
        |merged| {
            let Some(netsuke::cli::Commands::Build(args)) = merged.command else {
                anyhow::bail!("expected merged command to be build");
            };
            ensure!(
                !args.targets.is_empty(),
                "--default-target should be retained for build",
            );
            Ok(())
        },
    )
}

#[rstest]
#[case(json!({ "no_input": false }), ExpectedValidationError::InteractiveInput)]
#[case(
    json!({ "jobs": 65 }),
    ExpectedValidationError::JobsOutOfRange,
)]
fn cli_config_rejects_conflicting_or_unsupported_settings(
    default_cli_json: Result<serde_json::Value>,
    #[case] file_layer: serde_json::Value,
    #[case] expected_error: ExpectedValidationError,
) -> Result<()> {
    assert_merge_rejects(default_cli_json?, file_layer, expected_error)
}
