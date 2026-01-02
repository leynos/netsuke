//! Command line interface definition using clap.
//!
//! This module defines the [`Cli`] structure and its subcommands.
//! It mirrors the design described in `docs/netsuke-design.md`.

use clap::{ArgMatches, Args, CommandFactory, FromArgMatches, Parser, Subcommand};
use ortho_config::LanguageIdentifier;
use ortho_config::declarative::LayerComposition;
use ortho_config::figment::{Figment, providers::Env};
use ortho_config::localize_clap_error_with_command;
use ortho_config::uncased::Uncased;
use ortho_config::{
    CliValueExtractor, ConfigDiscovery, LocalizationArgs, Localizer, MergeComposer, OrthoConfig,
    OrthoMergeExt, OrthoResult, sanitize_value,
};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::path::PathBuf;
use std::str::FromStr;

use crate::host_pattern::HostPattern;

/// Maximum number of jobs accepted by the CLI.
const MAX_JOBS: usize = 64;
const CONFIG_ENV_VAR: &str = "NETSUKE_CONFIG_PATH";
const ENV_PREFIX: &str = "NETSUKE_";

fn parse_jobs(s: &str) -> Result<usize, String> {
    let value: usize = s
        .parse()
        .map_err(|_| format!("{s} is not a valid number"))?;
    if (1..=MAX_JOBS).contains(&value) {
        Ok(value)
    } else {
        Err(format!("jobs must be between 1 and {MAX_JOBS}"))
    }
}

/// Parse and normalise a URI scheme provided via CLI flags.
///
/// Schemes must begin with an ASCII letter and may contain ASCII letters,
/// digits, `+`, `-`, or `.` characters. The result is returned in lowercase.
fn parse_scheme(s: &str) -> Result<String, String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err(String::from("scheme must not be empty"));
    }
    let mut chars = trimmed.chars();
    if !chars.next().is_some_and(|c| c.is_ascii_alphabetic()) {
        return Err(format!("scheme '{s}' must start with an ASCII letter"));
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')) {
        return Err(format!("invalid scheme '{s}'"));
    }
    Ok(trimmed.to_ascii_lowercase())
}

fn parse_locale(s: &str) -> Result<String, String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err(String::from("locale must not be empty"));
    }
    LanguageIdentifier::from_str(trimmed)
        .map(|_| trimmed.to_owned())
        .map_err(|_| format!("invalid locale '{s}'"))
}

/// Parse a host pattern supplied via CLI flags.
///
/// The returned [`HostPattern`] retains both the wildcard flag and the
/// normalised host body so downstream configuration can reuse the parsed
/// structure without reparsing strings.
fn parse_host_pattern(s: &str) -> Result<HostPattern, String> {
    HostPattern::parse(s).map_err(|err| err.to_string())
}

/// A modern, friendly build system that uses YAML and Jinja, powered by Ninja.
#[derive(Debug, Parser, Serialize, Deserialize, OrthoConfig)]
#[command(author, version, about, long_about = None)]
#[ortho_config(prefix = "NETSUKE")]
pub struct Cli {
    /// Path to the Netsuke manifest file to use.
    #[arg(short, long, value_name = "FILE", default_value = "Netsukefile")]
    #[ortho_config(default = default_manifest_path(), cli_default_as_absent)]
    pub file: PathBuf,

    /// Run as if started in this directory.
    ///
    /// This affects manifest lookup, output paths, and config discovery.
    #[arg(short = 'C', long, value_name = "DIR")]
    pub directory: Option<PathBuf>,

    /// Set the number of parallel build jobs.
    ///
    /// Values must be between 1 and 64.
    #[arg(short, long, value_name = "N", value_parser = parse_jobs)]
    pub jobs: Option<usize>,

    /// Enable verbose diagnostic logging.
    #[arg(short, long)]
    #[ortho_config(default = false, cli_default_as_absent)]
    pub verbose: bool,

    /// Locale tag for CLI copy (for example: en-US, es-ES).
    #[arg(long, value_name = "LOCALE", value_parser = parse_locale)]
    pub locale: Option<String>,

    /// Additional URL schemes allowed for the `fetch` helper.
    #[arg(
        long = "fetch-allow-scheme",
        value_name = "SCHEME",
        value_parser = parse_scheme
    )]
    #[ortho_config(merge_strategy = "append")]
    pub fetch_allow_scheme: Vec<String>,

    /// Hostnames that are permitted when default deny is enabled.
    ///
    /// Supports wildcards such as `*.example.com`.
    #[arg(
        long = "fetch-allow-host",
        value_name = "HOST",
        value_parser = parse_host_pattern
    )]
    #[ortho_config(merge_strategy = "append")]
    pub fetch_allow_host: Vec<HostPattern>,

    /// Hostnames that are always blocked, even when allowed elsewhere.
    ///
    /// Supports wildcards such as `*.example.com`.
    #[arg(
        long = "fetch-block-host",
        value_name = "HOST",
        value_parser = parse_host_pattern
    )]
    #[ortho_config(merge_strategy = "append")]
    pub fetch_block_host: Vec<HostPattern>,

    /// Deny all hosts by default; only allow the declared allowlist.
    #[arg(long = "fetch-default-deny")]
    #[ortho_config(default = false, cli_default_as_absent)]
    pub fetch_default_deny: bool,

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

fn default_manifest_path() -> PathBuf {
    PathBuf::from("Netsukefile")
}

fn usage_body(usage: &str) -> &str {
    usage.strip_prefix("Usage: ").unwrap_or(usage)
}

fn localize_command(mut command: clap::Command, localizer: &dyn Localizer) -> clap::Command {
    let rendered_usage = command.render_usage().to_string();
    let fallback_usage = usage_body(&rendered_usage).to_owned();
    let mut args = LocalizationArgs::default();
    args.insert("binary", command.get_name().to_owned().into());
    args.insert("usage", fallback_usage.clone().into());
    let usage = localizer.message("cli.usage", Some(&args), &fallback_usage);
    command = command.override_usage(usage);

    if let Some(about) = command.get_about().map(ToString::to_string) {
        let localized_text = localizer.message("cli.about", None, &about);
        command = command.about(localized_text);
    } else if let Some(message) = localizer.lookup("cli.about", None) {
        command = command.about(message);
    }

    if let Some(long_about) = command.get_long_about().map(ToString::to_string) {
        let localized_text = localizer.message("cli.long_about", None, &long_about);
        command = command.long_about(localized_text);
    } else if let Some(message) = localizer.lookup("cli.long_about", None) {
        command = command.long_about(message);
    }

    localize_subcommands(&mut command, localizer);

    command
}

fn localize_subcommands(command: &mut clap::Command, localizer: &dyn Localizer) {
    for subcommand in command.get_subcommands_mut() {
        let name = subcommand.get_name().to_owned();
        let mut updated = std::mem::take(subcommand);
        let about_key = format!("cli.subcommand.{name}.about");
        if let Some(about) = updated.get_about().map(ToString::to_string) {
            let message = localizer.message(&about_key, None, &about);
            updated = updated.about(message);
        } else if let Some(message) = localizer.lookup(&about_key, None) {
            updated = updated.about(message);
        }

        let long_key = format!("cli.subcommand.{name}.long_about");
        if let Some(long_about) = updated.get_long_about().map(ToString::to_string) {
            let message = localizer.message(&long_key, None, &long_about);
            updated = updated.long_about(message);
        } else if let Some(message) = localizer.lookup(&long_key, None) {
            updated = updated.long_about(message);
        }

        *subcommand = updated;
    }
}

/// Inspect raw arguments and extract the `--locale` value when present.
#[must_use]
pub fn locale_hint_from_args(args: &[OsString]) -> Option<String> {
    let mut hint = None;
    let mut iter = args.iter().peekable();
    while let Some(arg) = iter.next() {
        let text = arg.to_string_lossy();
        if text == "--" {
            break;
        }
        if text == "--locale" {
            let Some(next) = iter.peek() else {
                break;
            };
            let next_text = next.to_string_lossy();
            if next_text == "--" {
                break;
            }
            hint = Some(next_text.into_owned());
            iter.next();
            continue;
        }
        if let Some(value) = text.strip_prefix("--locale=") {
            hint = Some(value.to_owned());
        }
    }
    hint
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
    localizer: &dyn Localizer,
) -> Result<(Cli, ArgMatches), clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let mut command = localize_command(Cli::command(), localizer);
    let mut matches = command
        .try_get_matches_from_mut(iter)
        .map_err(|err| localize_clap_error_with_command(err, localizer, Some(&command)))?;
    let cli = Cli::from_arg_matches_mut(&mut matches).map_err(|clap_err| {
        let with_cmd = clap_err.with_cmd(&command);
        localize_clap_error_with_command(with_cmd, localizer, Some(&command))
    })?;
    Ok((cli, matches))
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
fn is_empty_value(value: &serde_json::Value) -> bool {
    matches!(value, serde_json::Value::Object(map) if map.is_empty())
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

    match cli.extract_user_provided(matches) {
        Ok(value) if !is_empty_value(&value) => composer.push_cli(value),
        Ok(_) => {}
        Err(err) => errors.push(err),
    }

    let composition = LayerComposition::new(composer.layers(), errors);
    let mut merged = composition.into_merge_result(Cli::merge_from_layers)?;
    merged.command = command;
    Ok(merged)
}
