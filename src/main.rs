//! Application entry point.
//!
//! Parses command-line arguments and delegates execution to [`runner::run`].

use miette::Report;
use netsuke::{cli, cli_localization, locale_resolution, localization, output_prefs, runner};
use std::ffi::OsString;
use std::io::{self, Write};
use std::process::ExitCode;
use std::sync::Arc;
use tracing::Level;
use tracing_subscriber::fmt;

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().collect();
    let env = locale_resolution::SystemEnv;
    let system_locale = locale_resolution::SysLocale;
    let startup_locale = locale_resolution::resolve_startup_locale(&args, &env, &system_locale);
    let localizer = Arc::from(cli_localization::build_localizer(startup_locale.as_deref()));
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
    let runtime_locale = locale_resolution::resolve_runtime_locale(&merged_cli, &system_locale);
    let runtime_localizer = Arc::from(cli_localization::build_localizer(runtime_locale.as_deref()));
    localization::set_localizer(Arc::clone(&runtime_localizer));

    let max_level = if merged_cli.verbose {
        Level::DEBUG
    } else {
        Level::ERROR
    };
    init_tracing(max_level);

    let prefs = output_prefs::resolve(merged_cli.no_emoji);
    match runner::run(&merged_cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            let prefix = prefs.error_prefix();
            match err.downcast::<runner::RunnerError>() {
                Ok(runner_err) => {
                    let report = Report::new(runner_err);
                    drop(writeln!(io::stderr(), "{prefix} {report:?}"));
                }
                Err(other_err) => {
                    drop(writeln!(io::stderr(), "{prefix} {other_err}"));
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
