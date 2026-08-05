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
//! module (plus [`super::config`] and [`super::validation`]) to obtain
//! `Cli::command()` for man-page generation; anything reachable from here is
//! also compiled by the build script, so behaviour that the man page does not
//! need belongs in a sibling module instead.

use clap::{Args, Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::config::CliConfig;
use super::{AccessibilityPolicy, ColourPolicy, EmojiPolicy, ProgressPolicy};
use crate::host_pattern::HostPattern;

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
