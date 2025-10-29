#![cfg_attr(
    docsrs,
    expect(dead_code, reason = "conditional items for docs builds")
)]
//! Command line interface definition using clap.
//!
//! This module defines the [`Cli`] structure and its subcommands.
//! It mirrors the design described in `docs/netsuke-design.md`.

use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

use crate::host_pattern::normalise_host_pattern;

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
    match normalise_host_pattern(s) {
        Ok((normalised, wildcard)) => {
            if wildcard {
                Ok(format!("*.{normalised}"))
            } else {
                Ok(normalised)
            }
        }
        Err(err) => Err(err.to_string()),
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
    /// Apply the default command if none was specified.
    #[cfg_attr(doc, expect(dead_code, reason = "exercised by Default impl"))]
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
            file: PathBuf::from("Netsukefile"),
            directory: None,
            jobs: None,
            verbose: false,
            fetch_allow_scheme: Vec::new(),
            fetch_allow_host: Vec::new(),
            fetch_block_host: Vec::new(),
            fetch_default_deny: false,
            command: None,
        }
        .with_default_command()
    }
}

#[cfg(docsrs)]
const _: fn() = || {
    let _ = Cli::default().with_default_command();
};

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
