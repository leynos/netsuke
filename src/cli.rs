//! Command line interface definition using clap.
//!
//! This module defines the [`Cli`] structure and its subcommands.
//! It mirrors the design described in `docs/netsuke-design.md`.

use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

/// Maximum number of jobs accepted by the CLI.
const MAX_JOBS: usize = 64;

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

fn parse_scheme(s: &str) -> Result<String, String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err(String::from("scheme must not be empty"));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    {
        return Err(format!("invalid scheme '{s}'"));
    }
    Ok(trimmed.to_ascii_lowercase())
}

fn parse_host_pattern(s: &str) -> Result<String, String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err(String::from("host pattern must not be empty"));
    }
    if trimmed.contains("://") {
        return Err(format!("host pattern '{s}' must not include a scheme"));
    }
    if trimmed.contains('/') {
        return Err(format!("host pattern '{s}' must not contain '/'"));
    }

    let (wildcard, body) = if let Some(suffix) = trimmed.strip_prefix("*.") {
        if suffix.is_empty() {
            return Err(format!("wildcard host pattern '{s}' must include a suffix"));
        }
        (true, suffix)
    } else {
        (false, trimmed)
    };

    let normalised = body.to_ascii_lowercase();
    for label in normalised.split('.') {
        if label.is_empty() {
            return Err(format!("host pattern '{s}' must not contain empty labels"));
        }
        if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(format!("host pattern '{s}' contains invalid characters"));
        }
        if label.starts_with('-') || label.ends_with('-') {
            return Err(format!(
                "host pattern '{s}' must not start or end labels with '-'"
            ));
        }
    }

    if wildcard {
        Ok(format!("*.{normalised}"))
    } else {
        Ok(normalised)
    }
}

/// A modern, friendly build system that uses YAML and Jinja, powered by Ninja.
#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Path to the Netsuke manifest file to use.
    #[arg(short, long, value_name = "FILE", default_value = "Netsukefile")]
    pub file: PathBuf,

    /// Change to this directory before doing anything.
    #[arg(short = 'C', long, value_name = "DIR")]
    pub directory: Option<PathBuf>,

    /// Set the number of parallel build jobs.
    #[arg(short, long, value_name = "N", value_parser = parse_jobs)]
    pub jobs: Option<usize>,

    /// Enable verbose logging output.
    #[arg(short, long)]
    pub verbose: bool,

    /// Additional URL schemes allowed for the `fetch` helper.
    #[arg(
        long = "fetch-allow-scheme",
        value_name = "SCHEME",
        value_parser = parse_scheme
    )]
    pub fetch_allow_scheme: Vec<String>,

    /// Hostnames that must be explicitly allowed for network access.
    #[arg(
        long = "fetch-allow-host",
        value_name = "HOST",
        value_parser = parse_host_pattern
    )]
    pub fetch_allow_host: Vec<String>,

    /// Hostnames that are always blocked, even when allowed elsewhere.
    #[arg(
        long = "fetch-block-host",
        value_name = "HOST",
        value_parser = parse_host_pattern
    )]
    pub fetch_block_host: Vec<String>,

    /// Deny all hosts by default; only allow the declared allowlist.
    #[arg(long = "fetch-default-deny")]
    pub fetch_default_deny: bool,

    /// Optional subcommand to execute; defaults to `build` when omitted.
    #[command(subcommand)]
    pub command: Option<Commands>,
}

impl Cli {
    /// Parse command-line arguments, providing `build` as the default command.
    #[must_use]
    pub fn parse_with_default() -> Self {
        Self::parse().with_default_command()
    }

    /// Parse the provided arguments, applying the default command when needed.
    ///
    /// # Panics
    ///
    /// Panics if argument parsing fails.
    #[must_use]
    pub fn parse_from_with_default<I, T>(args: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        Self::try_parse_from(args)
            .unwrap_or_else(|e| panic!("CLI parsing failed: {e}"))
            .with_default_command()
    }

    /// Apply the default command if none was specified.
    #[must_use]
    fn with_default_command(mut self) -> Self {
        if self.command.is_none() {
            self.command = Some(Commands::Build(BuildArgs {
                emit: None,
                targets: Vec::new(),
            }));
        }
        self
    }
}

/// Arguments accepted by the `build` command.
#[derive(Debug, Args, PartialEq, Eq, Clone)]
pub struct BuildArgs {
    /// Write the generated Ninja manifest to this path and retain it.
    #[arg(long, value_name = "FILE")]
    pub emit: Option<PathBuf>,

    /// A list of specific targets to build.
    pub targets: Vec<String>,
}

/// Available top-level commands for Netsuke.
#[derive(Debug, Subcommand, PartialEq, Eq, Clone)]
pub enum Commands {
    /// Build specified targets (or default targets if none are given) `default`.
    Build(BuildArgs),

    /// Remove build artefacts and intermediate files.
    Clean,

    /// Display the build dependency graph in DOT format for visualization.
    Graph,

    /// Write the Ninja manifest to the specified file without invoking Ninja.
    Manifest {
        /// Output path for the generated Ninja file.
        #[arg(value_name = "FILE")]
        file: PathBuf,
    },
}
