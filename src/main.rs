//! Application entry point.
//!
//! Parses command-line arguments and delegates execution to [`runner::run`].

use netsuke::{cli::Cli, runner};
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = Cli::parse_with_default();
    if cli.verbose {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .init();
    }
    match runner::run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}
