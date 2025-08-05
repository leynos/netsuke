//! Application entry point.
//!
//! Parses command-line arguments and delegates execution to [`runner::run`].

use netsuke::{cli::Cli, runner};
use std::process::ExitCode;
use tracing::Level;
use tracing_subscriber::fmt;

fn main() -> ExitCode {
    let cli = Cli::parse_with_default();
    if cli.verbose {
        fmt().with_max_level(Level::DEBUG).init();
    }
    match runner::run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}
