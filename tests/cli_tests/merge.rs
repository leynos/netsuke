//! Configuration merge tests.
//!
//! These tests validate `OrthoConfig` layer precedence (defaults, file, env,
//! CLI) and list-value appending.
use super::merge_probe::{isolated_environment, merge_in_child};
use anyhow::{Context, Result, ensure};
use netsuke::cli::{CliConfig, ProgressPolicy};
use ortho_config::{MergeComposer, sanitize_value};
use rstest::{fixture, rstest};
use serde_json::{Map, Value, json};
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

#[derive(Debug, Clone, Copy)]
struct PrecedenceLayer {
    jobs: u8,
    fetch_allow_scheme: &'static str,
    env_allow_var: &'static str,
    env_block_var: &'static str,
    progress: &'static str,
    json: bool,
    file: Option<&'static str>,
    locale: Option<&'static str>,
    verbose: bool,
}

impl PrecedenceLayer {
    fn into_json(self) -> Value {
        let mut values: Map<String, Value> = [
            ("jobs", json!(self.jobs)),
            ("fetch_allow_scheme", json!([self.fetch_allow_scheme])),
            ("env_allow_var", json!([self.env_allow_var])),
            ("env_block_var", json!([self.env_block_var])),
            ("progress", json!(self.progress)),
            ("json", json!(self.json)),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value))
        .collect();
        if let Some(file) = self.file {
            values.insert("file".to_owned(), json!(file));
        }
        if let Some(locale) = self.locale {
            values.insert("locale".to_owned(), json!(locale));
        }
        if self.verbose {
            values.insert("verbose".to_owned(), json!(true));
        }
        Value::Object(values)
    }
}

const FILE_PRECEDENCE_LAYER: PrecedenceLayer = PrecedenceLayer {
    jobs: 2,
    fetch_allow_scheme: "http",
    env_allow_var: "FILE_ALLOW",
    env_block_var: "FILE_BLOCK",
    progress: "never",
    json: true,
    file: Some("Configfile"),
    locale: Some("en-US"),
    verbose: false,
};

const ENVIRONMENT_PRECEDENCE_LAYER: PrecedenceLayer = PrecedenceLayer {
    jobs: 3,
    fetch_allow_scheme: "ftp",
    env_allow_var: "ENV_ALLOW",
    env_block_var: "ENV_BLOCK",
    progress: "always",
    json: false,
    file: None,
    locale: None,
    verbose: false,
};

const CLI_PRECEDENCE_LAYER: PrecedenceLayer = PrecedenceLayer {
    jobs: 4,
    fetch_allow_scheme: "git",
    env_allow_var: "CLI_ALLOW",
    env_block_var: "CLI_BLOCK",
    progress: "never",
    json: true,
    file: None,
    locale: None,
    verbose: true,
};

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

fn defaults_with_precedence_values(mut defaults: serde_json::Value) -> Result<serde_json::Value> {
    let defaults_object = defaults
        .as_object_mut()
        .context("defaults should be an object")?;
    defaults_object.insert("jobs".to_owned(), json!(1));
    defaults_object.insert("fetch_allow_scheme".to_owned(), json!(["https"]));
    defaults_object.insert("env_allow_var".to_owned(), json!(["DEFAULT_ALLOW"]));
    defaults_object.insert("env_block_var".to_owned(), json!(["DEFAULT_BLOCK"]));
    defaults_object.insert("progress".to_owned(), json!("auto"));
    defaults_object.insert("json".to_owned(), json!(false));
    Ok(defaults)
}

fn merge_precedence_layers(defaults: serde_json::Value) -> Result<CliConfig> {
    let mut composer = MergeComposer::new();
    composer.push_defaults(defaults_with_precedence_values(defaults)?);
    composer.push_file(FILE_PRECEDENCE_LAYER.into_json(), None);
    composer.push_environment(ENVIRONMENT_PRECEDENCE_LAYER.into_json());
    composer.push_cli(CLI_PRECEDENCE_LAYER.into_json());
    CliConfig::merge_from_layers(composer.layers()).map_err(anyhow::Error::from)
}

fn assert_precedence_values(merged: &CliConfig) -> Result<()> {
    ensure!(
        merged.file.as_path() == Path::new("Configfile"),
        "file layer should override defaults",
    );
    ensure!(merged.jobs == Some(4), "CLI layer should override jobs");
    ensure!(
        merged.env_allow_var == ["DEFAULT_ALLOW", "FILE_ALLOW", "ENV_ALLOW", "CLI_ALLOW"],
        "environment allow variables should append in layer order",
    );
    ensure!(
        merged.env_block_var == ["DEFAULT_BLOCK", "FILE_BLOCK", "ENV_BLOCK", "CLI_BLOCK"],
        "environment block variables should append in layer order",
    );
    ensure!(
        merged.fetch_allow_scheme == vec!["https", "http", "ftp", "git"],
        "list values should append in layer order",
    );
    ensure!(
        merged.progress == ProgressPolicy::Never,
        "CLI layer should override progress setting",
    );
    ensure!(merged.json, "CLI layer should override json setting");
    ensure!(
        merged.locale.as_deref() == Some("en-US"),
        "file layer should populate locale when CLI does not override",
    );
    ensure!(merged.verbose, "CLI layer should set verbose");
    Ok(())
}

#[rstest]
fn cli_merge_layers_respects_precedence_and_appends_lists(
    default_cli_json: Result<serde_json::Value>,
) -> Result<()> {
    let merged = merge_precedence_layers(default_cli_json?)?;
    assert_precedence_values(&merged)
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
