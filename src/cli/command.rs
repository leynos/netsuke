//! The Clap-derived command tree.
//!
//! This module owns the runtime-visible [`Cli`] struct and every associated
//! Clap definition ([`InteractionArgs`], [`BuildArgs`], [`GraphArgs`],
//! [`Commands`]). It holds definitions only: no parsing entry point, no
//! localisation, and no runtime behaviour.
//!
//! **Pipeline position:** schema layer, below [`super::parser`].
//!
//! The narrow dependency surface is deliberate. `build.rs` recompiles this
//! module (plus [`super::config`], [`super::help`], and
//! [`super::validation`]) to obtain `Cli::command()` for man-page generation;
//! anything reachable from here is also compiled by the build script, so
//! behaviour that the man page does not need belongs in a sibling module
//! instead.

use camino::Utf8PathBuf;
use clap::{Args, Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::config::CliConfig;
use super::help::HelpArgs;
use super::{AccessibilityPolicy, ColourPolicy, EmojiPolicy, ProgressPolicy};
use crate::host_pattern::HostPattern;

/// A modern, friendly build system that uses YAML and Jinja, powered by Ninja.
#[derive(Debug, Parser, Serialize, Deserialize)]
#[command(
    name = "netsuke",
    bin_name = "netsuke",
    author,
    version,
    about,
    long_about = None,
    disable_help_subcommand = true
)]
pub struct Cli {
    /// Path to the Netsuke manifest file to use.
    #[arg(
        short,
        long,
        value_name = "FILE",
        default_value_os_t = CliConfig::default_manifest_path()
    )]
    pub file: Utf8PathBuf,

    /// Run as if started in this directory.
    ///
    /// This affects manifest lookup, output paths, and config discovery.
    #[arg(short = 'C', long, value_name = "DIR")]
    pub directory: Option<Utf8PathBuf>,

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

    /// Maximum `MiniJinja` instructions for one manifest evaluation.
    #[arg(long, value_name = "FUEL", default_value_t = 1_000_000)]
    pub manifest_evaluation_fuel: u64,

    /// Maximum `MiniJinja` instructions for one complete manifest.
    #[arg(long, value_name = "FUEL", default_value_t = 100_000_000)]
    pub manifest_fuel: u64,

    /// Maximum bytes emitted by one rendered manifest value.
    #[arg(long, value_name = "BYTES", default_value_t = 1_048_576)]
    pub manifest_rendered_value_bytes: usize,

    /// Maximum bytes emitted by every rendered manifest value.
    #[arg(long, value_name = "BYTES", default_value_t = 16_777_216)]
    pub manifest_rendered_manifest_bytes: usize,

    /// Maximum template and macro-import source bytes consumed by a manifest.
    #[arg(long, value_name = "BYTES", default_value_t = 4_194_304)]
    pub manifest_source_bytes: usize,

    /// Maximum values consumed from one manifest `foreach` iterator.
    #[arg(long, value_name = "COUNT", default_value_t = 10_000)]
    pub manifest_foreach_cardinality: usize,

    /// Maximum targets and actions emitted by manifest expansion.
    #[arg(long, value_name = "COUNT", default_value_t = 50_000)]
    pub manifest_expanded_entries: usize,

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
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::cli::{BuildArgs, Cli, Commands};
    ///
    /// let command = Cli::default().with_default_command().command;
    ///
    /// assert_eq!(command, Some(Commands::Build(BuildArgs::default())));
    /// ```
    #[must_use]
    pub fn with_default_command(mut self) -> Self {
        if self.command.is_none() {
            self.command = Some(Commands::Build(BuildArgs::default()));
        }
        self
    }
}

impl Default for Cli {
    /// Construct default CLI values with the `build` command selected.
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
            manifest_evaluation_fuel: 1_000_000,
            manifest_fuel: 100_000_000,
            manifest_rendered_value_bytes: 1_048_576,
            manifest_rendered_manifest_bytes: 16_777_216,
            manifest_source_bytes: 4_194_304,
            manifest_foreach_cardinality: 10_000,
            manifest_expanded_entries: 50_000,
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
    /// Construct interaction defaults that reject prompts unless explicitly enabled.
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

    /// Print the top-level help, or the help for a named topic such as `help targets`.
    ///
    /// With no topic this matches `--help`. `help targets` renders the
    /// target and action catalogue for the selected manifest.
    Help(HelpArgs),
}
