//! Clap-facing parser types and localisation helpers.
//!
//! This module owns the runtime-visible [`Cli`] struct and all associated
//! Clap definitions ([`BuildArgs`], [`Commands`]).  It also provides
//! [`parse_with_localizer_from`], which localises the Clap command, installs
//! localisation-aware [`LocalizedValueParser`] instances for every typed
//! argument, and returns `(Cli, ArgMatches)` for downstream processing.
//!
//! **Pipeline position:** parsing layer.
//!
//! - Receives raw `OsStr` arguments from the process entry point.
//! - Delegates value validation to [`super::parsing`] helpers.
//! - Returns a `Cli`/`ArgMatches` pair consumed by [`super::merge`].
//!
//! [`LocalizedValueParser`]: self::LocalizedValueParser

use clap::builder::{TypedValueParser, ValueParser};
use clap::error::ErrorKind;
use clap::{ArgMatches, Args, CommandFactory, FromArgMatches, Parser, Subcommand};
use ortho_config::localize_clap_error_with_command;
use ortho_config::{LocalizationArgs, Localizer};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;

use super::config::CliConfig;
use super::parsing::{
    parse_accessibility_policy, parse_color_policy, parse_emoji_policy, parse_host_pattern,
    parse_jobs, parse_locale, parse_progress_policy, parse_scheme,
};
use super::{AccessibilityPolicy, ColourPolicy, EmojiPolicy, ProgressPolicy};
use crate::cli_l10n::localize_command;
pub use crate::cli_l10n::{json_hint_from_args, locale_hint_from_args};
use crate::host_pattern::HostPattern;
use crate::theme::ThemePreference;

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
    #[arg(
        short,
        long,
        value_name = "FILE",
        default_value_os_t = CliConfig::default_manifest_path()
    )]
    pub file: PathBuf,

    /// Run as if started in this directory.
    ///
    /// This affects manifest lookup, output paths, and config discovery.
    #[arg(short = 'C', long, value_name = "DIR")]
    pub directory: Option<PathBuf>,

    /// Path to a configuration file, bypassing automatic discovery.
    #[arg(long, value_name = "FILE")]
    #[serde(skip)]
    pub config: Option<PathBuf>,

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

    /// Emit machine-readable JSON output.
    #[arg(long)]
    pub json: bool,

    /// Interaction policy flags.
    #[command(flatten)]
    pub interaction: InteractionArgs,

    /// Select the colour policy for terminal output.
    #[arg(long, value_name = "POLICY", default_value_t)]
    pub color: ColourPolicy,

    /// Select the emoji policy for terminal output.
    #[arg(long, value_name = "POLICY", default_value_t)]
    pub emoji: EmojiPolicy,

    /// Select the progress-rendering policy.
    #[arg(long, value_name = "POLICY", default_value_t)]
    pub progress: ProgressPolicy,

    /// Select the accessible-output policy.
    #[arg(long, value_name = "POLICY", default_value_t)]
    pub accessibility: AccessibilityPolicy,

    /// Default build targets used when none are specified on the CLI.
    #[arg(long = "default-target", value_name = "TARGET")]
    pub default_targets: Vec<String>,

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

    /// Return the effective theme preference for emoji policy resolution.
    #[must_use]
    pub const fn theme_preference(&self) -> Option<ThemePreference> {
        match self.emoji {
            EmojiPolicy::Auto => None,
            EmojiPolicy::Always => Some(ThemePreference::Unicode),
            EmojiPolicy::Never => Some(ThemePreference::Ascii),
        }
    }

    /// Return an explicit accessible-output override, if configured.
    #[must_use]
    pub const fn accessibility_override(&self) -> Option<bool> {
        match self.accessibility {
            AccessibilityPolicy::Auto => None,
            AccessibilityPolicy::On => Some(true),
            AccessibilityPolicy::Off => Some(false),
        }
    }

    /// Return whether interactive input is disabled.
    #[must_use]
    pub const fn no_input(&self) -> bool {
        self.interaction.no_input
    }

    /// Return whether progress summaries should be enabled.
    #[must_use]
    pub const fn progress_enabled(&self) -> bool {
        !matches!(self.progress, ProgressPolicy::Never)
    }
}

impl Default for Cli {
    fn default() -> Self {
        Self {
            file: CliConfig::default_manifest_path(),
            directory: None,
            config: None,
            jobs: None,
            verbose: false,
            locale: None,
            fetch_allow_scheme: Vec::new(),
            fetch_allow_host: Vec::new(),
            fetch_block_host: Vec::new(),
            fetch_default_deny: false,
            json: false,
            interaction: InteractionArgs::default(),
            color: ColourPolicy::Auto,
            emoji: EmojiPolicy::Auto,
            progress: ProgressPolicy::Auto,
            accessibility: AccessibilityPolicy::Auto,
            default_targets: Vec::new(),
            command: None,
        }
        .with_default_command()
    }
}

/// Arguments controlling whether Netsuke may read interactive input.
#[derive(Debug, Args, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct InteractionArgs {
    /// Never read interactive input.
    #[arg(long, default_value_t = true)]
    pub no_input: bool,
}

impl Default for InteractionArgs {
    fn default() -> Self {
        Self { no_input: true }
    }
}

/// Arguments accepted by the `build` command.
#[derive(Debug, Args, PartialEq, Eq, Clone, Serialize, Deserialize, Default)]
pub struct BuildArgs {
    /// A list of specific targets to build.
    #[serde(default)]
    pub targets: Vec<String>,
}

/// Arguments accepted by the `graph` command.
///
/// `html` and `output` are per-invocation flags and are intentionally excluded
/// from `OrthoConfig` layering (`#[serde(skip)]`); layering them through a
/// configuration file would silently change the artefact destination.
#[derive(Debug, Args, PartialEq, Eq, Clone, Serialize, Deserialize, Default)]
pub struct GraphArgs {
    /// Render the graph as a self-contained HTML page instead of DOT.
    #[arg(long)]
    #[serde(skip)]
    pub html: bool,

    /// Write the graph artefact to FILE. Use `-` for stdout.
    #[arg(long, value_name = "FILE")]
    #[serde(skip)]
    pub output: Option<PathBuf>,
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
    Graph(GraphArgs),

    /// Generate the Ninja manifest without invoking Ninja.
    Generate {
        /// Write the generated Ninja manifest to FILE instead of stdout.
        #[arg(long, value_name = "FILE")]
        output: Option<PathBuf>,
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
    let color_policy_parser = LocalizedValueParser::new(Arc::clone(localizer), parse_color_policy);
    let emoji_policy_parser = LocalizedValueParser::new(Arc::clone(localizer), parse_emoji_policy);
    let progress_policy_parser =
        LocalizedValueParser::new(Arc::clone(localizer), parse_progress_policy);
    let accessibility_policy_parser =
        LocalizedValueParser::new(Arc::clone(localizer), parse_accessibility_policy);

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
    command = command.mut_arg("color", |arg| {
        arg.value_parser(ValueParser::new(color_policy_parser))
    });
    command = command.mut_arg("emoji", |arg| {
        arg.value_parser(ValueParser::new(emoji_policy_parser))
    });
    command = command.mut_arg("progress", |arg| {
        arg.value_parser(ValueParser::new(progress_policy_parser))
    });
    command = command.mut_arg("accessibility", |arg| {
        arg.value_parser(ValueParser::new(accessibility_policy_parser))
    });
    command
}

/// Maximum number of jobs accepted by the CLI.
pub(super) const MAX_JOBS: usize = 64;
