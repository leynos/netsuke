//! Command line interface definition using clap.
//!
//! This module defines the [`Cli`] structure and its subcommands.
//! It mirrors the design described in `docs/netsuke-design.md`.

use clap::{Parser, Subcommand};
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
            self.command = Some(Commands::Build {
                targets: Vec::new(),
            });
        }
        self
    }
}

/// Available top-level commands for Netsuke.
#[derive(Debug, Subcommand, PartialEq, Eq, Clone)]
pub enum Commands {
    /// Build specified targets (or default targets if none are given) [default].
    Build {
        /// A list of specific targets to build.
        targets: Vec<String>,
    },

    /// Remove build artifacts and intermediate files.
    Clean,

    /// Display the build dependency graph in DOT format for visualization.
    Graph,
}
