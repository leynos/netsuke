//! Application entry point.
//!
//! Parses command-line arguments and delegates execution to [`runner::run`].
//! It records configuration-load outcomes and latency during startup.

use clap::error::ErrorKind;
use clap::{ArgMatches, ColorChoice};
use miette::Report;
use netsuke::{
    cli, cli_localization, diagnostic_json,
    display_plan::{DisplayPlan, TerminalFacts, resolve_display_plan},
    ir::IrGenError,
    locale_resolution, localization,
    output_mode::EnvSignals,
    output_prefs, runner,
};
use ortho_config::Localizer;
use std::ffi::OsString;
use std::io::{self, IsTerminal, Write};
use std::process::ExitCode;
use std::sync::Arc;
use tracing_subscriber::filter::LevelFilter;
// Keep the subscriber extension visible to src/tests.rs through `use super::*`.
#[cfg(test)]
use tracing_subscriber::layer::SubscriberExt;

use main_diagnostics::DiagMode;
use monotony::{MonotonicClock, StdMonotonicClock};

mod observability;

mod main_tracing;

mod main_diagnostics;

mod startup_tracing;

#[cfg(test)]
pub mod test_tracing_capture;

mod config_load;
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

/// Send buffered startup diagnostics where `mode` says they belong.
///
/// Human mode releases them to stderr; JSON mode drops them, so the diagnostic
/// document is the only thing on that stream.
fn settle_startup_diagnostics(writer: &StartupWriter, mode: DiagMode) {
    if mode.is_json() {
        writer.discard();
    } else if let Err(err) = writer.release_to_stderr() {
        // Nothing better to do: the channel for reporting this is the one that
        // just failed.
        drop(writeln!(
            io::stderr(),
            "failed to flush startup diagnostics: {err}"
        ));
    }
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
    let startup_signals = resolve_startup_signals(&args, dependencies);
    let terminal_facts = startup_signals.terminal_facts;
    // Recorded at `WARN` but written to a buffer, not to stderr. The JSON hint
    // is only a hint — configuration can still turn JSON on — and the JSON
    // diagnostic goes to stderr, so an event emitted now could corrupt it.
    // Buffering keeps the locale fallback report without taking that risk.
    let startup_mode = DiagMode::from_json_enabled(startup_signals.json_hint);
    observability::init_metrics();
    let localizer = startup_localizer(&args, dependencies.locale_env, dependencies.system_locale);
    let startup_parse = StartupParse {
        args,
        localizer: &localizer,
        mode: startup_mode,
        color_choice: startup_signals.parser_color,
        startup_writer,
    };
    let (parsed_cli, matches) = match parse_cli_or_exit(startup_parse) {
        Ok(parsed) => parsed,
        // The buffer was settled inside, before the branch that exits.
        Err(code) => return code,
    };
    let verbose = parsed_cli.verbose;
    let startup_plan = resolve_display_plan(parsed_cli.display_policies(), terminal_facts);
    main_tracing::set_tracing_ansi(startup_plan.stderr_colour());
    if is_informational_help(&parsed_cli) {
        settle_startup_diagnostics(startup_writer, startup_mode);
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
    let merged_verbose = merged_cli.verbose;
    let runtime_mode = DiagMode::from_json_enabled(merged_cli.json);

    let exit_code = run_configured_command(
        &merged_cli,
        dependencies.system_locale,
        runtime_mode,
        terminal_facts,
    );
    finish_run(exit_code, merged_verbose)
}

/// Signals resolved from raw argv and the environment before configuration loads.
struct StartupSignals {
    /// Whether raw argv already asks for JSON, before configuration is merged.
    json_hint: bool,
    /// The terminal facts shared by the startup and runtime display plans.
    terminal_facts: TerminalFacts,
    /// Colour choice for Clap's own help and version renderers.
    parser_color: ColorChoice,
}

/// Resolve the terminal facts, JSON hint, and help colour from startup inputs.
///
/// All three depend only on raw argv and the environment, so none of them may
/// wait for configuration to load: Clap's help renderer needs its colour choice
/// before the parser runs, and the JSON hint decides how diagnostics recorded
/// during that same window are buffered.
fn resolve_startup_signals<L, S, C, E>(
    args: &[OsString],
    dependencies: &RunWithArgsDependencies<'_, L, S, C, E>,
) -> StartupSignals
where
    L: locale_resolution::LocaleEnvProvider,
    S: locale_resolution::SystemLocale,
    C: MonotonicClock,
    E: cli::ConfigEnvProvider,
{
    let json_hint = locale_resolution::resolve_startup_json(args, dependencies.locale_env);
    let terminal_facts = TerminalFacts::from_signals(
        EnvSignals::read(|key| dependencies.locale_env.var(key)),
        dependencies.stdout_is_terminal,
        dependencies.stderr_is_terminal,
    );
    let colour_hint = locale_resolution::resolve_startup_colour(args, dependencies.locale_env);
    let colour_for_help = netsuke::display_plan::startup_colour(colour_hint, terminal_facts);
    let parser_color = if requests_help_or_version(args) && colour_for_help {
        ColorChoice::Always
    } else {
        ColorChoice::Never
    };
    StartupSignals {
        json_hint,
        terminal_facts,
        parser_color,
    }
}

/// Resolve the runtime display plan, configure the process, and run the command.
///
/// Resolution happens once here so the tracing formatter, the status reporter,
/// the diagnostic hook, and the runner all act on the same plan rather than
/// each deriving colour and glyph decisions from ambient state.
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
    configure_runtime(merged_cli, system_locale, runtime_mode, plan);
    match runner::run(merged_cli, plan) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => handle_runner_error(err, plan.prefs(), runtime_mode),
    }
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

/// Return whether Clap's help or version renderer was requested in raw argv.
fn requests_help_or_version(args: &[OsString]) -> bool {
    for arg in args {
        let text = arg.to_string_lossy();
        if text == "--" {
            break;
        }
        if matches!(text.as_ref(), "-h" | "--help" | "-V" | "--version") {
            return true;
        }
    }
    false
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

/// Choose the stderr verbosity for `mode`.
///
/// JSON mode silences tracing entirely so stderr carries only the diagnostic
/// document. `--verbose` selects `TRACE` because the `NETSUKE_CONFIG` lookup is
/// traced at that level.
///
/// Otherwise `WARN`: a run that falls back to English, or loads a catalogue
/// that fails to parse, reports it at that level, and `ERROR` would leave both
/// silent — which is the condition a user would report as a bug.
const fn startup_filter(mode: DiagMode, verbose: bool) -> LevelFilter {
    if mode.is_json() {
        LevelFilter::OFF
    } else if verbose {
        LevelFilter::TRACE
    } else {
        LevelFilter::WARN
    }
}

/// Resolve and install the localizer used while parsing startup arguments.
fn startup_localizer(
    args: &[OsString],
    env: &impl locale_resolution::LocaleEnvProvider,
    system_locale: &impl locale_resolution::SystemLocale,
) -> Arc<dyn Localizer> {
    let startup_locale = locale_resolution::resolve_startup_locale(args, env, system_locale);
    let localizer = Arc::from(cli_localization::build_localizer(startup_locale.as_deref()));
    localization::set_localizer(Arc::clone(&localizer));
    localizer
}

/// Everything startup knows before the command line has been parsed.
///
/// Grouped rather than passed positionally so the parser's colour choice and
/// the diagnostic mode cannot be transposed at a call site: the two are both
/// small enums that read as "some policy" in isolation.
struct StartupParse<'a> {
    /// Raw process arguments, including the program name.
    args: Vec<OsString>,
    /// Localizer built from the startup locale, before configuration merging.
    localizer: &'a Arc<dyn Localizer>,
    /// Diagnostic mode assumed before configuration can override it.
    mode: DiagMode,
    /// Colour choice handed to clap for help and version rendering.
    color_choice: ColorChoice,
    /// Buffer holding startup diagnostics until a path decides their stream.
    startup_writer: &'a StartupWriter,
}

/// Parse localized arguments, emitting valid JSON diagnostics when JSON is
/// selected.
///
/// Help and version requests exit through clap; all other parse failures return
/// the corresponding failure [`ExitCode`] on the JSON path.
fn parse_cli_or_exit(parse: StartupParse<'_>) -> Result<(cli::Cli, ArgMatches), ExitCode> {
    let StartupParse {
        args,
        localizer,
        mode,
        color_choice,
        startup_writer,
    } = parse;
    match cli::parse_with_localizer_from(args, localizer, color_choice) {
        Ok(parsed) => Ok(parsed),
        Err(err) => {
            // Every arm below terminates the process or returns, and
            // `Error::exit` never returns, so the buffered startup
            // diagnostics have to be settled here. Configuration is never
            // read on these paths, so `mode` is the effective mode.
            settle_startup_diagnostics(startup_writer, mode);
            if matches!(
                err.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) {
                err.exit();
            }
            if mode.is_json() {
                Err(diagnostic_json::emit_or_fallback(
                    diagnostic_json::render_error_json(&err),
                ))
            } else {
                err.exit();
            }
        }
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
