//! Command line interface definition using clap.
//!
//! This module defines the [`Cli`] structure and its subcommands.
//! It mirrors the design described in `docs/netsuke-design.md`.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

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
    #[arg(short, long, value_name = "N")]
    pub jobs: Option<usize>,

    #[command(subcommand)]
    pub command: Option<Commands>,
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
    Clean {},

    /// Display the build dependency graph in DOT format for visualization.
    Graph {},
}
