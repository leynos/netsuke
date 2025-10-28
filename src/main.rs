//! Application entry point.
//!
//! Parses command-line arguments and delegates execution to [`runner::run`].

use clap::Parser;
use netsuke::{cli::Cli, runner};
use std::process::ExitCode;
use tracing::Level;
use tracing_subscriber::fmt;

fn main() -> ExitCode {
    let cli = Cli::parse().with_default_command();
    let max_level = if cli.verbose {
        Level::DEBUG
    } else {
        Level::ERROR
    };
    fmt().with_max_level(max_level).init();
    match runner::run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!(error = %err, "runner failed");
            ExitCode::FAILURE
        }
    }
}
