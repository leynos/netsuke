//! Netsuke command line entry point.
//!
//! This module defines the `Cli` struct and `Commands` enum using `clap` to
//! parse user input. The build command is treated as the default when no
//! subcommand is provided.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Top-level command line options.
#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Path to the Netsuke manifest file to use.
    #[arg(short, long, value_name = "FILE", default_value = "Netsukefile")]
    file: PathBuf,

    /// Change to this directory before doing anything.
    #[arg(long)]
    directory: Option<PathBuf>,

    /// Set the number of parallel build jobs.
    #[arg(short, long, value_name = "N")]
    jobs: Option<usize>,

    #[command(subcommand)]
    command: Option<Commands>,
}

/// Available subcommands.
#[derive(Debug, Subcommand, PartialEq, Eq)]
enum Commands {
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

fn main() {
    let cli = Cli::parse();
    let command = cli.command.unwrap_or(Commands::Build { targets: vec![] });
    match command {
        Commands::Build { targets } => {
            println!("Build invoked with {targets:?}");
            // pipeline will be added here
        }
        Commands::Clean {} => println!("Clean invoked"),
        Commands::Graph {} => println!("Graph invoked"),
    }
}
