//! Application entry point.
//!
//! Parses command-line arguments and delegates execution to [`runner::run`].
//! It records configuration-load outcomes and latency during startup.

use miette::Report;
use netsuke::{
    cli, cli_localization, diagnostic_json,
    display_plan::{DisplayPlan, TerminalFacts, resolve_display_plan},
    ir::IrGenError,
    locale_resolution, localization, output_prefs, runner,
};
use std::ffi::OsString;
use std::io::{self, IsTerminal, Write};
use std::process::ExitCode;
use tracing_subscriber::filter::LevelFilter;
// Keep the subscriber extension visible to src/tests.rs through `use super::*`.
#[cfg(test)]
use tracing_subscriber::layer::SubscriberExt;

use main_diagnostics::DiagMode;
use monotony::{MonotonicClock, StdMonotonicClock};
use std::sync::Arc;

mod observability;

mod main_tracing;

mod main_diagnostics;

mod startup_tracing;

#[cfg(test)]
pub mod test_tracing_capture;

mod config_load;

mod main_startup;
use main_startup::{
    StartupParse, StartupSignals, parse_cli_or_exit, settle_startup_diagnostics, startup_filter,
    startup_localizer,
};
use startup_tracing::StartupWriter;

/// Injectable dependencies for one invocation of the startup composition root.
///
/// Production supplies process-backed adapters and captured stream facts,
/// while tests inject deterministic environment and terminal values.
struct RunWithArgsDependencies<'a, L, S, C, E>
where
    L: locale_resolution::LocaleEnvProvider,
    S: locale_resolution::SystemLocale,
    C: MonotonicClock,
    E: cli::ConfigEnvProvider,
{
    /// Environment provider used for locale resolution.
    locale_env: &'a L,
    /// System-locale provider used for default-language detection.
    system_locale: &'a S,
    /// Monotonic clock used to time configuration loading.
    configuration_clock: &'a C,
    /// Environment provider consulted during configuration discovery and merge.
    config_env: &'a E,
    /// Whether stdout was connected to a terminal at process entry.
    stdout_is_terminal: bool,
    /// Whether stderr was connected to a terminal at process entry.
    stderr_is_terminal: bool,
}

/// Collect production arguments and environment providers, then run Netsuke.
#[expect(
    clippy::disallowed_methods,
    reason = "the process entrypoint captures each terminal stream once"
)]
fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().collect();
    let env = locale_resolution::SystemEnv;
    let system_locale = locale_resolution::SysLocale;
    let config_env = cli::ConfigStdEnvProvider;
    let configuration_clock = StdMonotonicClock;
    let dependencies = RunWithArgsDependencies {
        locale_env: &env,
        system_locale: &system_locale,
        configuration_clock: &configuration_clock,
        config_env: &config_env,
        stdout_is_terminal: io::stdout().is_terminal(),
        stderr_is_terminal: io::stderr().is_terminal(),
    };
    let startup_writer = StartupWriter::buffering();
    main_tracing::init_tracing(LevelFilter::WARN, startup_writer.clone());
    if let Err(error) = main_diagnostics::install_miette_handler() {
        tracing::error!(error = %error, "failed to install diagnostic renderer");
        drop(startup_writer.release_to_stderr());
        return ExitCode::FAILURE;
    }
    run_with_args(args, &dependencies, &startup_writer)
}

/// Run one invocation with injectable locale, configuration, and clock providers.
///
/// Uses the injected [`MonotonicClock`] to orchestrate timed configuration
/// loading before running the selected command. Returns the command's process
/// exit code, including failures from argument parsing, configuration loading,
/// and runner execution. The production entrypoint owns process-wide tracing
/// initialization so tests can call this function with a scoped subscriber.
fn run_with_args<L, S, C, E>(
    args: Vec<OsString>,
    dependencies: &RunWithArgsDependencies<'_, L, S, C, E>,
    startup_writer: &StartupWriter,
) -> ExitCode
where
    L: locale_resolution::LocaleEnvProvider,
    S: locale_resolution::SystemLocale,
    C: MonotonicClock,
    E: cli::ConfigEnvProvider,
{
    let window = match run_startup(args, dependencies, startup_writer) {
        Ok(window) => window,
        // The startup buffer was settled inside, before the branch that exits.
        Err(code) => return code,
    };
    let terminal_facts = window.startup_signals.terminal_facts;
    let startup_mode = window.mode;
    let parsed_cli = window.parsed_cli;
    let matches = window.matches;
    let verbose = parsed_cli.verbose;
    let startup_plan = resolve_display_plan(
        window
            .startup_signals
            .policies_before_configuration(&parsed_cli),
        terminal_facts,
    );
    main_tracing::set_tracing_ansi(startup_plan.stderr_colour());
    if is_informational_help(&parsed_cli) {
        settle_startup_diagnostics(startup_writer, startup_mode);
        // The runner-rendered help topics never reach the configuration merge,
        // so this route applies the startup plan's colour rather than
        // re-deriving one from configuration it deliberately has not read.
        return finish_run(
            run_cli(
                &parsed_cli,
                dependencies.system_locale,
                startup_mode,
                startup_plan,
            ),
            verbose,
        );
    }

    let configuration = config_load::ConfigurationLoadContext {
        parsed_cli: &parsed_cli,
        matches: &matches,
        startup_mode,
        startup_writer,
        config_env: dependencies.config_env,
    };
    let merged_cli = match config_load::resolve_configuration(
        &configuration,
        dependencies.configuration_clock,
    ) {
        Ok(merged) => merged,
        Err(code) => return finish_run(code, verbose),
    };
    run_merged_command(&merged_cli, dependencies.system_locale, terminal_facts)
}

/// Run the startup command and return the pre-configuration window it built.
///
/// This is the run's first stage. It is a command, not a query: it installs the
/// global metrics recorder and the process localizer, and parsing may itself
/// end the run — `--help`, `--version`, and parse failures all render and exit —
/// so the outcome is a `Result` whose error is the code already returned to the
/// shell, not a failure the caller can inspect.
///
/// The name says "startup" rather than "resolve" because of those effects. A
/// caller reading `resolve_startup_window` would reasonably expect a read-only
/// computation over the arguments.
fn run_startup<L, S, C, E>(
    args: Vec<OsString>,
    dependencies: &RunWithArgsDependencies<'_, L, S, C, E>,
    startup_writer: &StartupWriter,
) -> Result<StartupWindow, ExitCode>
where
    L: locale_resolution::LocaleEnvProvider,
    S: locale_resolution::SystemLocale,
    C: MonotonicClock,
    E: cli::ConfigEnvProvider,
{
    let startup_signals = StartupSignals::resolve(
        &args,
        dependencies.locale_env,
        dependencies.stdout_is_terminal,
        dependencies.stderr_is_terminal,
    );
    // Recorded at `WARN` but written to a buffer, not to stderr. The JSON hint
    // is only a hint — configuration can still turn JSON on — and the JSON
    // diagnostic goes to stderr, so an event emitted now could corrupt it.
    // Buffering keeps the locale fallback report without taking that risk.
    let mode = DiagMode::from_json_enabled(startup_signals.json_hint);
    observability::init_metrics();
    let localizer = startup_localizer(&args, dependencies.locale_env, dependencies.system_locale);
    // Parsing may itself end the run, so its exit code is propagated as the
    // window's error rather than inspected here.
    let (parsed_cli, matches) = parse_cli_or_exit(StartupParse {
        args,
        localizer: &localizer,
        mode,
        color_choice: startup_signals.parser_color,
        startup_writer,
    })?;
    Ok(StartupWindow {
        startup_signals,
        mode,
        parsed_cli,
        matches,
    })
}

/// The values the startup window produces before configuration is merged.
struct StartupWindow {
    /// Terminal facts and argv-derived hints captured at process entry.
    startup_signals: StartupSignals,
    /// Diagnostics mode the startup buffer was created in.
    mode: DiagMode,
    /// The parsed command line, which the window does not re-derive.
    parsed_cli: cli::Cli,
    /// Raw clap matches retained for the configuration merge.
    matches: clap::ArgMatches,
}

/// Run the command after the configuration merge has settled its own mode.
///
/// Resolution happens here so the tracing formatter, the status reporter, the
/// diagnostic hook, and the runner all act on one plan. The JSON mode comes
/// from the merged configuration, so it is read here rather than at startup.
fn run_merged_command<S>(
    merged_cli: &cli::Cli,
    system_locale: &S,
    terminal_facts: TerminalFacts,
) -> ExitCode
where
    S: locale_resolution::SystemLocale,
{
    let runtime_mode = DiagMode::from_json_enabled(merged_cli.json);
    let exit_code = run_configured_command(merged_cli, system_locale, runtime_mode, terminal_facts);
    finish_run(exit_code, merged_cli.verbose)
}

/// Resolve the runtime display plan, then run the command under it.
///
/// Resolution happens once here so the tracing formatter, the status reporter,
/// the diagnostic hook, and the runner all act on the same plan rather than
/// each deriving colour and glyph decisions from ambient state. The runtime
/// sequence itself belongs to [`run_cli`]; this function only supplies the plan
/// from the merged configuration, so the two entry points cannot drift apart.
fn run_configured_command<S>(
    merged_cli: &cli::Cli,
    system_locale: &S,
    runtime_mode: DiagMode,
    terminal_facts: TerminalFacts,
) -> ExitCode
where
    S: locale_resolution::SystemLocale,
{
    let plan = resolve_display_plan(merged_cli.display_policies(), terminal_facts);
    run_cli(merged_cli, system_locale, runtime_mode, plan)
}

/// Emit a development snapshot after the command has completed when requested.
fn finish_run(exit_code: ExitCode, verbose: bool) -> ExitCode {
    if verbose {
        observability::emit_metrics_snapshot();
    }
    exit_code
}

/// Return whether the parsed CLI asks only for informational help.
///
/// Most topic-specific help is informational, but [`cli::HelpTopic::Targets`] is
/// excluded because it may require configuration. General help may also
/// require configuration, so it is not counted here.
const fn is_informational_help(cli: &cli::Cli) -> bool {
    matches!(
        &cli.command,
        Some(cli::Commands::Help(args))
            if !matches!(args.topic.as_ref(), Some(cli::HelpTopic::Targets(_)))
    )
}

/// Configure the runtime and dispatch the selected command through the runner.
fn run_cli(
    cli: &cli::Cli,
    system_locale: &impl locale_resolution::SystemLocale,
    runtime_mode: DiagMode,
    plan: DisplayPlan,
) -> ExitCode {
    configure_runtime(cli, system_locale, runtime_mode, plan);
    match runner::run(cli, plan) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => handle_runner_error(err, plan.prefs(), runtime_mode),
    }
}

/// Apply the effective output filter and install the runtime localizer.
fn configure_runtime(
    merged_cli: &cli::Cli,
    system_locale: &impl locale_resolution::SystemLocale,
    mode: DiagMode,
    plan: DisplayPlan,
) {
    main_tracing::set_tracing_ansi(plan.stderr_colour());
    main_diagnostics::set_style(plan.diagnostic_style());
    // Raised before the localizer is built, so a fallback warning is both
    // visible in a normal run and suppressed in JSON mode, where stderr
    // carries the diagnostic document.
    main_tracing::set_tracing_filter(startup_filter(mode, merged_cli.verbose));

    let runtime_locale = locale_resolution::resolve_runtime_locale(merged_cli, system_locale);
    let runtime_localizer = Arc::from(cli_localization::build_localizer(runtime_locale.as_deref()));
    localization::set_localizer(Arc::clone(&runtime_localizer));
}

/// Render a runner failure according to the selected human or JSON mode.
///
/// JSON output uses the stable diagnostic serializer and fallback payload to
/// remain valid JSON; human output writes a formatted error to stderr. Both
/// paths return failure.
fn handle_runner_error(
    err: anyhow::Error,
    prefs: output_prefs::OutputPrefs,
    mode: DiagMode,
) -> ExitCode {
    if mode.is_json() {
        return diagnostic_json::emit_or_fallback(main_diagnostics::render_runtime_error_json(
            &err,
        ));
    }
    let prefix = prefs.error_prefix();
    if let Some(cycle_error) = circular_dependency_cause(&err) {
        tracing::error!(error_kind = "circular_dependency", "runner failed");
        drop(writeln!(io::stderr(), "{prefix} {cycle_error}"));
        return ExitCode::FAILURE;
    }
    match err.downcast::<runner::RunnerError>() {
        Ok(runner_err) => {
            let report = Report::new(runner_err);
            drop(writeln!(io::stderr(), "{prefix} {report:?}"));
        }
        Err(other_err) => {
            tracing::error!(error = %other_err, "runner failed");
            drop(writeln!(io::stderr(), "{prefix} {other_err}"));
        }
    }
    ExitCode::FAILURE
}

/// Find a circular-dependency error inside a contextual runner failure.
fn circular_dependency_cause(error: &anyhow::Error) -> Option<&IrGenError> {
    error.chain().find_map(|cause| {
        cause
            .downcast_ref::<IrGenError>()
            .filter(|ir_error| matches!(ir_error, IrGenError::CircularDependency { .. }))
    })
}

#[cfg(test)]
mod tests;
