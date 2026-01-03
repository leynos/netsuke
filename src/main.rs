//! Application entry point.
//!
//! Parses command-line arguments and delegates execution to [`runner::run`].

use netsuke::{cli, cli_localization, runner};
use std::ffi::OsString;
use std::io;
use std::process::ExitCode;
use tracing::Level;
use tracing_subscriber::fmt;

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().collect();
    let locale_hint = cli::locale_hint_from_args(&args);
    let env_locale = std::env::var("NETSUKE_LOCALE").ok();
    let locale = locale_hint.as_deref().or(env_locale.as_deref());
    let localizer = cli_localization::build_localizer(locale);

    let (parsed_cli, matches) = match cli::parse_with_localizer_from(args, localizer.as_ref()) {
        Ok(parsed) => parsed,
        Err(err) => err.exit(),
    };

    let merged_cli = match cli::merge_with_config(&parsed_cli, &matches) {
        Ok(merged) => merged.with_default_command(),
        Err(err) => {
            init_tracing(Level::ERROR);
            tracing::error!(error = %err, "configuration load failed");
            return ExitCode::FAILURE;
        }
    };

    let max_level = if merged_cli.verbose {
        Level::DEBUG
    } else {
        Level::ERROR
    };
    init_tracing(max_level);

    match runner::run(&merged_cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!(error = %err, "runner failed");
            ExitCode::FAILURE
        }
    }
}

fn init_tracing(max_level: Level) {
    fmt()
        .with_max_level(max_level)
        .with_writer(io::stderr)
        .init();
}
