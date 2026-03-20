//! Command line interface definition using clap.
//!
//! This module defines the [`Cli`] structure and its subcommands.
//! It mirrors the design described in `docs/netsuke-design.md`.

use clap::{ArgMatches, Args, CommandFactory, FromArgMatches, Parser, Subcommand};
use ortho_config::localize_clap_error_with_command;
use ortho_config::{LocalizationArgs, Localizer, OrthoConfig};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;

use crate::cli_l10n::localize_command;
use crate::host_pattern::HostPattern;
use crate::theme::ThemePreference;
mod config_merge;
mod parsing;
mod validation;

use config_merge::default_manifest_path;
pub use config_merge::{merge_with_config, resolve_merged_diag_json};
use validation::configure_validation_parsers;

/// Maximum number of jobs accepted by the CLI.
const MAX_JOBS: usize = 64;
const CONFIG_ENV_VAR: &str = "NETSUKE_CONFIG_PATH";
const ENV_PREFIX: &str = "NETSUKE_";

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

    /// Enable verbose diagnostic logging and completion timing summaries.
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

    /// CLI theme preset (auto, unicode, ascii).
    #[arg(long, value_name = "THEME")]
    pub theme: Option<ThemePreference>,

    /// Emit machine-readable diagnostics in JSON on stderr.
    #[arg(long)]
    #[ortho_config(default = false)]
    pub diag_json: bool,

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
            theme: None,
            diag_json: false,
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

/// Inspect raw arguments and extract the requested locale before full parsing.
#[must_use]
pub fn locale_hint_from_args(args: &[OsString]) -> Option<String> {
    crate::cli_l10n::locale_hint_from_args(args)
}

/// Inspect raw arguments and extract the requested `--diag-json` state.
#[must_use]
pub fn diag_json_hint_from_args(args: &[OsString]) -> Option<bool> {
    crate::cli_l10n::diag_json_hint_from_args(args)
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
