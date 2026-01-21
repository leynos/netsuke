//! Application entry point.
//!
//! Parses command-line arguments and delegates execution to [`runner::run`].

use miette::Report;
use netsuke::{cli, cli_localization, localization, runner};
use std::ffi::OsString;
use std::io::{self, Write};
use std::process::ExitCode;
use std::sync::Arc;
use tracing::Level;
use tracing_subscriber::fmt;

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().collect();
    let locale_hint = cli::locale_hint_from_args(&args);
    let env_locale = std::env::var("NETSUKE_LOCALE").ok();
    let locale = locale_hint.as_deref().or(env_locale.as_deref());
    let localizer = Arc::from(cli_localization::build_localizer(locale));
    localization::set_localizer(Arc::clone(&localizer));

    let (parsed_cli, matches) = match cli::parse_with_localizer_from(args, &localizer) {
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
    let runtime_locale = merged_cli
        .locale
        .as_deref()
        .or(locale_hint.as_deref())
        .or(env_locale.as_deref());
    let runtime_localizer = Arc::from(cli_localization::build_localizer(runtime_locale));
    localization::set_localizer(Arc::clone(&runtime_localizer));

    let max_level = if merged_cli.verbose {
        Level::DEBUG
    } else {
        Level::ERROR
    };
    init_tracing(max_level);

    match runner::run(&merged_cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            // Check if the error is a RunnerError with diagnostic info.
            match err.downcast::<runner::RunnerError>() {
                Ok(runner_err) => {
                    let report = Report::new(runner_err);
                    drop(writeln!(io::stderr(), "{report:?}"));
                }
                Err(other_err) => {
                    tracing::error!(error = %other_err, "runner failed");
                }
            }
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
