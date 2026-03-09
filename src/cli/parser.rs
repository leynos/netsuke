//! Clap-facing parser types and localized parsing helpers.

use clap::builder::{TypedValueParser, ValueParser};
use clap::error::ErrorKind;
use clap::{ArgMatches, Args, CommandFactory, FromArgMatches, Parser, Subcommand};
use ortho_config::localize_clap_error_with_command;
use ortho_config::{LocalizationArgs, Localizer};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;

use super::parsing::{parse_host_pattern, parse_jobs, parse_locale, parse_scheme};
use super::{ColourPolicy, OutputFormat, SpinnerMode, Theme};
pub use crate::cli_l10n::{diag_json_hint_from_args, locale_hint_from_args};
use crate::cli_l10n::localize_command;
use crate::host_pattern::HostPattern;

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

pub(super) fn validation_message(
    localizer: &dyn Localizer,
    key: &'static str,
    args: Option<&LocalizationArgs<'_>>,
    fallback: &str,
) -> String {
    localizer.message(key, args, fallback)
}

/// A modern, friendly build system that uses YAML and Jinja, powered by Ninja.
#[derive(Debug, Parser, Serialize, Deserialize)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Path to the Netsuke manifest file to use.
    #[arg(short, long, value_name = "FILE", default_value = "Netsukefile")]
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

    /// Enable verbose diagnostic logging and completion timing summaries.
    #[arg(short, long)]
    pub verbose: bool,

    /// Locale tag for CLI copy (for example: en-US, es-ES).
    #[arg(long, value_name = "LOCALE")]
    pub locale: Option<String>,

    /// Additional URL schemes allowed for the `fetch` helper.
    #[arg(long = "fetch-allow-scheme", value_name = "SCHEME")]
    pub fetch_allow_scheme: Vec<String>,

    /// Hostnames that are permitted when default deny is enabled.
    ///
    /// Supports wildcards such as `*.example.com`.
    #[arg(long = "fetch-allow-host", value_name = "HOST")]
    pub fetch_allow_host: Vec<HostPattern>,

    /// Hostnames that are always blocked, even when allowed elsewhere.
    ///
    /// Supports wildcards such as `*.example.com`.
    #[arg(long = "fetch-block-host", value_name = "HOST")]
    pub fetch_block_host: Vec<HostPattern>,

    /// Deny all hosts by default; only allow the declared allowlist.
    #[arg(long = "fetch-default-deny")]
    pub fetch_default_deny: bool,

    /// Force accessible output mode on or off (overrides auto-detection).
    #[arg(long)]
    pub accessible: Option<bool>,

    /// Suppress emoji glyphs in output (overrides auto-detection).
    #[arg(long)]
    pub no_emoji: Option<bool>,

    /// Emit machine-readable diagnostics in JSON on stderr.
    #[arg(long)]
    pub diag_json: bool,

    /// Force standard progress summaries on or off.
    ///
    /// When omitted, Netsuke enables progress summaries in standard mode.
    #[arg(long)]
    pub progress: Option<bool>,

    /// Resolved colour policy from layered configuration.
    #[arg(skip)]
    pub colour_policy: Option<ColourPolicy>,

    /// Resolved spinner mode from layered configuration.
    #[arg(skip)]
    pub spinner_mode: Option<SpinnerMode>,

    /// Resolved output format from layered configuration.
    #[arg(skip)]
    pub output_format: Option<OutputFormat>,

    /// Resolved presentation theme from layered configuration.
    #[arg(skip)]
    pub theme: Option<Theme>,

    /// Optional subcommand to execute; defaults to `build` when omitted.
    #[serde(skip)]
    #[command(subcommand)]
    pub command: Option<Commands>,
}

impl Cli {
    /// Apply the default command if none was specified.
    #[must_use]
    pub fn with_default_command(mut self) -> Self {
        if self.command.is_none() {
            self.command = Some(Commands::Build(BuildArgs::default()));
        }
        self
    }

    /// Return the effective emoji override for output preference resolution.
    #[must_use]
    pub const fn no_emoji_override(&self) -> Option<bool> {
        if matches!(self.theme, Some(Theme::Ascii)) || matches!(self.no_emoji, Some(true)) {
            Some(true)
        } else {
            self.no_emoji
        }
    }

    /// Return whether progress summaries should be enabled.
    #[must_use]
    pub const fn progress_enabled(&self) -> bool {
        match (self.progress, self.spinner_mode) {
            (Some(value), _) => value,
            (None, Some(SpinnerMode::Disabled)) => false,
            _ => true,
        }
    }
}

impl Default for Cli {
    fn default() -> Self {
        Self {
            file: PathBuf::from("Netsukefile"),
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
            diag_json: false,
            colour_policy: None,
            spinner_mode: None,
            output_format: None,
            theme: None,
            command: None,
        }
        .with_default_command()
    }
}

/// Arguments accepted by the `build` command.
#[derive(Debug, Args, PartialEq, Eq, Clone, Serialize, Deserialize, Default)]
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
    let matches_for_merge = matches.clone();
    let mut matches_for_parse = matches;
    let cli = Cli::from_arg_matches_mut(&mut matches_for_parse).map_err(|clap_err| {
        let with_cmd = clap_err.with_cmd(&command);
        localize_clap_error_with_command(with_cmd, localizer.as_ref(), Some(&command))
    })?;
    Ok((cli, matches_for_merge))
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
/// Maximum number of jobs accepted by the CLI.
pub(super) const MAX_JOBS: usize = 64;
