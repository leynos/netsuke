//! Command line interface definition using clap.
//!
//! This module defines the [`Cli`] structure and its subcommands.
//! It mirrors the design described in `docs/netsuke-design.md`.

use clap::parser::ValueSource;
use clap::{ArgMatches, Args, CommandFactory, FromArgMatches, Parser, Subcommand};
use ortho_config::LanguageIdentifier;
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
use std::str::FromStr;
use std::sync::Arc;

pub use crate::cli_l10n::locale_hint_from_args;
use crate::cli_l10n::localize_command;
use crate::host_pattern::HostPattern;
use crate::localization::{self, keys};

/// Maximum number of jobs accepted by the CLI.
const MAX_JOBS: usize = 64;
const CONFIG_ENV_VAR: &str = "NETSUKE_CONFIG_PATH";
const ENV_PREFIX: &str = "NETSUKE_";

fn validation_message(
    key: &'static str,
    args: Option<&LocalizationArgs<'_>>,
    fallback: &str,
) -> String {
    localization::localizer().message(key, args, fallback)
}

/// Set the localizer used for CLI validation errors.
pub fn set_validation_localizer(localizer: Arc<dyn Localizer>) {
    localization::set_localizer(localizer);
}

/// Compile-time assertion that `set_validation_localizer` has the expected signature.
const _: fn(Arc<dyn Localizer>) = set_validation_localizer;

fn parse_jobs(s: &str) -> Result<usize, String> {
    let value: usize = s.parse().map_err(|_| {
        let mut args = LocalizationArgs::default();
        args.insert("value", s.to_owned().into());
        validation_message(
            keys::CLI_JOBS_INVALID_NUMBER,
            Some(&args),
            &format!("{s} is not a valid number"),
        )
    })?;
    if (1..=MAX_JOBS).contains(&value) {
        Ok(value)
    } else {
        let mut args = LocalizationArgs::default();
        args.insert("min", 1.to_string().into());
        args.insert("max", MAX_JOBS.to_string().into());
        Err(validation_message(
            keys::CLI_JOBS_OUT_OF_RANGE,
            Some(&args),
            &format!("jobs must be between 1 and {MAX_JOBS}"),
        ))
    }
}

/// Parse and normalise a URI scheme provided via CLI flags.
///
/// Schemes must begin with an ASCII letter and may contain ASCII letters,
/// digits, `+`, `-`, or `.` characters. The result is returned in lowercase.
fn parse_scheme(s: &str) -> Result<String, String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err(validation_message(
            keys::CLI_SCHEME_EMPTY,
            None,
            "scheme must not be empty",
        ));
    }
    let mut chars = trimmed.chars();
    if !chars.next().is_some_and(|c| c.is_ascii_alphabetic()) {
        let mut args = LocalizationArgs::default();
        args.insert("scheme", s.to_owned().into());
        return Err(validation_message(
            keys::CLI_SCHEME_INVALID_START,
            Some(&args),
            &format!("scheme '{s}' must start with an ASCII letter"),
        ));
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')) {
        let mut args = LocalizationArgs::default();
        args.insert("scheme", s.to_owned().into());
        return Err(validation_message(
            keys::CLI_SCHEME_INVALID,
            Some(&args),
            &format!("invalid scheme '{s}'"),
        ));
    }
    Ok(trimmed.to_ascii_lowercase())
}

fn parse_locale(s: &str) -> Result<String, String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err(validation_message(
            keys::CLI_LOCALE_EMPTY,
            None,
            "locale must not be empty",
        ));
    }
    LanguageIdentifier::from_str(trimmed)
        .map(|_| trimmed.to_owned())
        .map_err(|_| {
            let mut args = LocalizationArgs::default();
            args.insert("locale", trimmed.to_owned().into());
            validation_message(
                keys::CLI_LOCALE_INVALID,
                Some(&args),
                &format!("invalid locale '{trimmed}'"),
            )
        })
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
    #[arg(short, long, value_name = "N", value_parser = parse_jobs)]
    pub jobs: Option<usize>,

    /// Enable verbose diagnostic logging.
    #[arg(short, long)]
    #[ortho_config(default = false)]
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
    #[ortho_config(default = false)]
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
    localizer: &dyn Localizer,
) -> Result<(Cli, ArgMatches), clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let mut command = localize_command(Cli::command(), localizer);
    let matches = command
        .try_get_matches_from_mut(iter)
        .map_err(|err| localize_clap_error_with_command(err, localizer, Some(&command)))?;
    // Clone matches before from_arg_matches_mut consumes the values.
    let matches_for_merge = matches.clone();
    let mut matches_for_parse = matches;
    let cli = Cli::from_arg_matches_mut(&mut matches_for_parse).map_err(|clap_err| {
        let with_cmd = clap_err.with_cmd(&command);
        localize_clap_error_with_command(with_cmd, localizer, Some(&command))
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
            return Err(Arc::new(ortho_config::OrthoError::Validation {
                key: String::from("cli"),
                message: validation_message(
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
    ] {
        if matches.value_source(field) != Some(ValueSource::CommandLine) {
            map.remove(field);
        }
    }

    Ok(serde_json::Value::Object(map))
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
