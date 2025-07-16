//! CLI execution and command dispatch logic.
//!
//! This module keeps [`main`] minimal by providing a single entry point that
//! handles command execution. It currently prints which command was invoked.

use crate::cli::{Cli, Commands};

/// Execute the parsed [`Cli`] commands.
pub fn run(cli: Cli) {
    match cli.command.unwrap_or(Commands::Build {
        targets: Vec::new(),
    }) {
        Commands::Build { targets } => {
            println!("Building targets: {targets:?}");
        }
        Commands::Clean => {
            println!("Clean requested");
        }
        Commands::Graph => {
            println!("Graph requested");
        }
    }
}
