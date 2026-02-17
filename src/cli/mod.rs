//! Command line interface definition using clap.
//!
//! This module defines the [`Cli`] structure and its subcommands.
//! It mirrors the design described in `docs/netsuke-design.md`.

use clap::builder::{TypedValueParser, ValueParser};
use clap::error::ErrorKind;
use clap::parser::ValueSource;
use clap::{ArgMatches, Args, CommandFactory, FromArgMatches, Parser, Subcommand};
use ortho_config::declarative::LayerComposition;
use ortho_config::figment::{Figment, providers::Env};
use ortho_config::localize_clap_error_with_command;
use ortho_config::uncased::Uncased;
use ortho_config::{
    ConfigDiscovery, LocalizationArgs, Localizer, MergeComposer, OrthoConfig, OrthoMergeExt,
    OrthoResult, sanitize_value,
};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;

pub use crate::cli_l10n::locale_hint_from_args;
use crate::cli_l10n::localize_command;
use crate::host_pattern::HostPattern;
use crate::localization::{self, keys};
mod parsing;

use parsing::{parse_host_pattern, parse_jobs, parse_locale, parse_scheme};

/// Maximum number of jobs accepted by the CLI.
const MAX_JOBS: usize = 64;
const CONFIG_ENV_VAR: &str = "NETSUKE_CONFIG_PATH";
const ENV_PREFIX: &str = "NETSUKE_";

#[derive(Clone)]
struct LocalizedValueParser<F> {
    localizer: Arc<dyn Localizer>,
    parser: F,
}

impl<F> LocalizedValueParser<F> {
    fn new(localizer: Arc<dyn Localizer>, parser: F) -> Self {
        Self { localizer, parser }
    }
}

impl<F, T> TypedValueParser for LocalizedValueParser<F>
where
    F: Fn(&dyn Localizer, &str) -> Result<T, String> + Clone + Send + Sync + 'static,
    T: Send + Sync + Clone + 'static,
{
    type Value = T;

    fn parse_ref(
        &self,
        cmd: &clap::Command,
        _arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        let mut command = cmd.clone();
        let Some(raw_value) = value.to_str() else {
            return Err(command.error(ErrorKind::InvalidUtf8, "invalid UTF-8"));
        };
        (self.parser)(self.localizer.as_ref(), raw_value)
            .map_err(|err| command.error(ErrorKind::ValueValidation, err))
    }
}

fn validation_message(
    localizer: &dyn Localizer,
    key: &'static str,
    args: Option<&LocalizationArgs<'_>>,
    fallback: &str,
) -> String {
    localizer.message(key, args, fallback)
}

/// A modern, friendly build system that uses YAML and Jinja, powered by Ninja.
#[derive(Debug, Parser, Serialize, Deserialize, OrthoConfig)]
#[command(author, version, about, long_about = None)]
#[ortho_config(prefix = "NETSUKE")]
pub struct Cli {
    /// Path to the Netsuke manifest file to use.
    #[arg(short, long, value_name = "FILE", default_value = "Netsukefile")]
    #[ortho_config(default = default_manifest_path())]
    pub file: PathBuf,

    /// Run as if started in this directory.
    ///
    /// This affects manifest lookup, output paths, and config discovery.
    #[arg(short = 'C', long, value_name = "DIR")]
    pub directory: Option<PathBuf>,

    /// Set the number of parallel build jobs.
    ///
    /// Values must be between 1 and 64.
    #[arg(short, long, value_name = "N")]
    pub jobs: Option<usize>,

    /// Enable verbose diagnostic logging.
    #[arg(short, long)]
    #[ortho_config(default = false)]
    pub verbose: bool,

    /// Locale tag for CLI copy (for example: en-US, es-ES).
    #[arg(long, value_name = "LOCALE")]
    pub locale: Option<String>,

    /// Additional URL schemes allowed for the `fetch` helper.
    #[arg(long = "fetch-allow-scheme", value_name = "SCHEME")]
    #[ortho_config(merge_strategy = "append")]
    pub fetch_allow_scheme: Vec<String>,

    /// Hostnames that are permitted when default deny is enabled.
    ///
    /// Supports wildcards such as `*.example.com`.
    #[arg(long = "fetch-allow-host", value_name = "HOST")]
    #[ortho_config(merge_strategy = "append")]
    pub fetch_allow_host: Vec<HostPattern>,

    /// Hostnames that are always blocked, even when allowed elsewhere.
    ///
    /// Supports wildcards such as `*.example.com`.
    #[arg(long = "fetch-block-host", value_name = "HOST")]
    #[ortho_config(merge_strategy = "append")]
    pub fetch_block_host: Vec<HostPattern>,

    /// Deny all hosts by default; only allow the declared allowlist.
    #[arg(long = "fetch-default-deny")]
    #[ortho_config(default = false)]
    pub fetch_default_deny: bool,

    /// Force accessible output mode on or off (overrides auto-detection).
    #[arg(long)]
    pub accessible: Option<bool>,

    /// Suppress emoji glyphs in output (overrides auto-detection).
    #[arg(long)]
    pub no_emoji: Option<bool>,

    /// Force standard progress summaries on or off.
    ///
    /// When omitted, Netsuke enables progress summaries in standard mode.
    #[arg(long)]
    pub progress: Option<bool>,

    /// Optional subcommand to execute; defaults to `build` when omitted.
    ///
    /// `OrthoConfig` merging ignores this field; CLI parsing supplies it.
    #[serde(skip)]
    #[command(subcommand)]
    #[ortho_config(skip_cli)]
    pub command: Option<Commands>,
}

impl Cli {
    /// Apply the default command if none was specified.
    #[must_use]
    pub fn with_default_command(mut self) -> Self {
        if self.command.is_none() {
            self.command = Some(Commands::Build(BuildArgs {
                emit: None,
                targets: Vec::new(),
            }));
        }
        self
    }
}

impl Default for Cli {
    fn default() -> Self {
        Self {
            file: default_manifest_path(),
            directory: None,
            jobs: None,
            verbose: false,
            locale: None,
            fetch_allow_scheme: Vec::new(),
            fetch_allow_host: Vec::new(),
            fetch_block_host: Vec::new(),
            fetch_default_deny: false,
            accessible: None,
            progress: None,
            no_emoji: None,
            command: None,
        }
        .with_default_command()
    }
}

/// Arguments accepted by the `build` command.
#[derive(Debug, Args, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct BuildArgs {
    /// Write the generated Ninja manifest to this path and retain it.
    #[arg(long, value_name = "FILE")]
    pub emit: Option<PathBuf>,

    /// A list of specific targets to build.
    #[serde(default)]
    pub targets: Vec<String>,
}

/// Available top-level commands for Netsuke.
#[derive(Debug, Subcommand, PartialEq, Eq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Commands {
    /// Build specified targets (or default targets if none are given).
    Build(BuildArgs),

    /// Remove build artefacts and intermediate files.
    Clean,

    /// Display the build dependency graph in DOT format for visualisation.
    Graph,

    /// Write the Ninja manifest to the specified file without invoking Ninja.
    Manifest {
        /// Output path for the generated Ninja file.
        ///
        /// Use `-` to write to stdout.
        #[arg(value_name = "FILE")]
        file: PathBuf,
    },
}

/// Return the default manifest filename when none is provided.
fn default_manifest_path() -> PathBuf {
    PathBuf::from("Netsukefile")
}

/// Parse CLI arguments with localized clap output.
///
/// Returns both the parsed CLI struct and the `ArgMatches` required for
/// configuration merging.
///
/// # Errors
///
/// Returns a `clap::Error` with localization applied when parsing fails.
pub fn parse_with_localizer_from<I, T>(
    iter: I,
    localizer: &Arc<dyn Localizer>,
) -> Result<(Cli, ArgMatches), clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let mut command = localize_command(Cli::command(), localizer.as_ref());
    command = configure_validation_parsers(command, localizer);
    let matches = command
        .try_get_matches_from_mut(iter)
        .map_err(|err| localize_clap_error_with_command(err, localizer.as_ref(), Some(&command)))?;
    // Clone matches before from_arg_matches_mut consumes the values.
    let matches_for_merge = matches.clone();
    let mut matches_for_parse = matches;
    let cli = Cli::from_arg_matches_mut(&mut matches_for_parse).map_err(|clap_err| {
        let with_cmd = clap_err.with_cmd(&command);
        localize_clap_error_with_command(with_cmd, localizer.as_ref(), Some(&command))
    })?;
    Ok((cli, matches_for_merge))
}

/// Return the prefixed environment provider for CLI configuration.
fn env_provider() -> Env {
    Env::prefixed(ENV_PREFIX)
}

/// Build configuration discovery rooted in the optional working directory.
fn config_discovery(directory: Option<&PathBuf>) -> ConfigDiscovery {
    let mut builder = ConfigDiscovery::builder("netsuke").env_var(CONFIG_ENV_VAR);
    if let Some(dir) = directory {
        builder = builder.clear_project_roots().add_project_root(dir);
    }
    builder.build()
}

/// Return `true` when no CLI overrides were supplied.
///
/// The merge pipeline treats an empty JSON object as "no overrides".
fn is_empty_value(value: &serde_json::Value) -> bool {
    matches!(value, serde_json::Value::Object(map) if map.is_empty())
}

fn cli_overrides_from_matches(cli: &Cli, matches: &ArgMatches) -> OrthoResult<serde_json::Value> {
    let value = sanitize_value(cli)?;
    let mut map = match value {
        serde_json::Value::Object(map) => map,
        other => {
            let mut args = LocalizationArgs::default();
            args.insert("value", format!("{other:?}").into());
            let localizer = localization::localizer();
            return Err(Arc::new(ortho_config::OrthoError::Validation {
                key: String::from("cli"),
                message: validation_message(
                    localizer.as_ref(),
                    keys::CLI_CONFIG_EXPECTED_OBJECT,
                    Some(&args),
                    &format!("expected parsed CLI values to serialize to an object, got {other:?}",),
                ),
            }));
        }
    };

    map.remove("command");
    for field in [
        "file",
        "verbose",
        "fetch_default_deny",
        "fetch_allow_scheme",
        "fetch_allow_host",
        "fetch_block_host",
        "accessible",
        "progress",
        "no_emoji",
    ] {
        if matches.value_source(field) != Some(ValueSource::CommandLine) {
            map.remove(field);
        }
    }

    Ok(serde_json::Value::Object(map))
}

fn configure_validation_parsers(
    mut command: clap::Command,
    localizer: &Arc<dyn Localizer>,
) -> clap::Command {
    let jobs_parser = LocalizedValueParser::new(Arc::clone(localizer), parse_jobs);
    let locale_parser = LocalizedValueParser::new(Arc::clone(localizer), parse_locale);
    let scheme_parser = LocalizedValueParser::new(Arc::clone(localizer), parse_scheme);
    let host_parser = LocalizedValueParser::new(Arc::clone(localizer), parse_host_pattern);

    command = command.mut_arg("jobs", |arg| {
        arg.value_parser(ValueParser::new(jobs_parser))
    });
    command = command.mut_arg("locale", |arg| {
        arg.value_parser(ValueParser::new(locale_parser))
    });
    command = command.mut_arg("fetch_allow_scheme", |arg| {
        arg.value_parser(ValueParser::new(scheme_parser.clone()))
    });
    command = command.mut_arg("fetch_allow_host", |arg| {
        arg.value_parser(ValueParser::new(host_parser.clone()))
    });
    command = command.mut_arg("fetch_block_host", |arg| {
        arg.value_parser(ValueParser::new(host_parser))
    });
    command
}

/// Merge configuration layers over the parsed CLI values.
///
/// # Errors
///
/// Returns an [`ortho_config::OrthoError`] if layer composition or merging
/// fails.
pub fn merge_with_config(cli: &Cli, matches: &ArgMatches) -> OrthoResult<Cli> {
    let command = cli.command.clone();
    let mut errors = Vec::new();
    let mut composer = MergeComposer::with_capacity(4);

    match sanitize_value(&Cli::default()) {
        Ok(value) => composer.push_defaults(value),
        Err(err) => errors.push(err),
    }

    let discovery = config_discovery(cli.directory.as_ref());
    let mut file_layers = discovery.compose_layers();
    errors.append(&mut file_layers.required_errors);
    if file_layers.value.is_empty() {
        errors.append(&mut file_layers.optional_errors);
    }
    for layer in file_layers.value {
        composer.push_layer(layer);
    }

    let env_provider = env_provider()
        .map(|key| Uncased::new(key.as_str().to_ascii_uppercase()))
        .split("__");
    match Figment::from(env_provider)
        .extract::<serde_json::Value>()
        .into_ortho_merge()
    {
        Ok(value) => composer.push_environment(value),
        Err(err) => errors.push(err),
    }

    match cli_overrides_from_matches(cli, matches) {
        Ok(value) if !is_empty_value(&value) => composer.push_cli(value),
        Ok(_) => {}
        Err(err) => errors.push(err),
    }

    let composition = LayerComposition::new(composer.layers(), errors);
    let mut merged = composition.into_merge_result(Cli::merge_from_layers)?;
    merged.command = command;
    Ok(merged)
}
