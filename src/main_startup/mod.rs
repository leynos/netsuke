//! Startup signals and argument parsing, ahead of configuration merging.
//!
//! Netsuke has to know three things before it can parse its own command line:
//! which locale to render diagnostics in, whether raw argv already asked for
//! JSON, and what colour to give clap's help and version renderers. All three
//! depend only on raw argv and the environment, so none may wait for
//! configuration to load. This module owns that window — resolving the
//! signals, building the startup localizer, and turning a parse failure into
//! the exit code the process should return.
//!
//! Everything here is binary-private: it composes library seams, it is not one.

use clap::error::ErrorKind;
use clap::{ArgMatches, ColorChoice};
use ortho_config::Localizer;
use std::ffi::OsString;
use std::io::{self, Write};
use std::process::ExitCode;
use std::sync::Arc;
use tracing_subscriber::filter::LevelFilter;

use crate::main_diagnostics::DiagMode;
use crate::startup_tracing::StartupWriter;
use netsuke::{
    cli, cli_localization, diagnostic_json,
    display_plan::{DisplayPolicies, TerminalFacts, startup_colour},
    locale_resolution, localization,
    output_mode::EnvSignals,
};

/// Signals resolved from raw argv and the environment before configuration.
pub(crate) struct StartupSignals {
    /// Whether raw argv already asks for JSON, before configuration is merged.
    pub(crate) json_hint: bool,
    /// The terminal facts shared by the startup and runtime display plans.
    pub(crate) terminal_facts: TerminalFacts,
    /// The colour policy decided before configuration, from argv then
    /// `NETSUKE_COLOR`.
    ///
    /// The merged configuration resolves this again from the same two sources
    /// plus configuration files, so the startup window must apply the same
    /// precedence or a `NETSUKE_COLOR` value would style clap's `--help` while
    /// leaving the runner-rendered `help` and `help build` topics plain.
    pub(crate) colour_hint: cli::ColourPolicy,
    /// Colour choice for clap's own help and version renderers.
    pub(crate) parser_color: ColorChoice,
}

impl StartupSignals {
    /// Resolve the terminal facts, JSON hint, and help colour from startup inputs.
    ///
    /// `stdout_is_terminal` and `stderr_is_terminal` are captured once at
    /// process entry, because a later capture could observe a different stream.
    pub(crate) fn resolve(
        args: &[OsString],
        env: &impl locale_resolution::LocaleEnvProvider,
        stdout_is_terminal: bool,
        stderr_is_terminal: bool,
    ) -> Self {
        let json_hint = locale_resolution::resolve_startup_json(args, env);
        let terminal_facts = TerminalFacts::from_signals(
            EnvSignals::read(|key| env.var(key)),
            stdout_is_terminal,
            stderr_is_terminal,
        );
        let colour_hint = locale_resolution::resolve_startup_colour(args, env);
        let colour_for_help = startup_colour(colour_hint, terminal_facts);
        let parser_color = if requests_help_or_version(args) && colour_for_help {
            ColorChoice::Always
        } else {
            ColorChoice::Never
        };
        Self {
            json_hint,
            terminal_facts,
            colour_hint,
            parser_color,
        }
    }

    /// Collect the display policies for the window before configuration merges.
    ///
    /// The parsed command line cannot supply the colour policy by itself: its
    /// `auto` default is indistinguishable from an explicit `--color auto`, and
    /// `NETSUKE_COLOR` is folded in by the configuration merge this window
    /// deliberately runs ahead of. Substituting the resolved startup hint keeps
    /// the informational-help path — which never reaches the merge — on the
    /// same colour decision as every other path.
    pub(crate) const fn policies_before_configuration(&self, cli: &cli::Cli) -> DisplayPolicies {
        let parsed = cli.display_policies();
        DisplayPolicies {
            colour: self.colour_hint,
            emoji: parsed.emoji,
            progress: parsed.progress,
            accessibility: parsed.accessibility,
            json: parsed.json,
        }
    }
}

/// Send buffered startup diagnostics where `mode` says they belong.
///
/// Human mode releases them to stderr; JSON mode drops them, so the diagnostic
/// document is the only thing on that stream.
pub(crate) fn settle_startup_diagnostics(writer: &StartupWriter, mode: DiagMode) {
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

/// Choose the stderr verbosity for `mode`.
///
/// JSON mode silences tracing entirely so stderr carries only the diagnostic
/// document. `--verbose` selects `TRACE` because the `NETSUKE_CONFIG` lookup is
/// traced at that level.
///
/// Otherwise `WARN`: a run that falls back to English, or loads a catalogue
/// that fails to parse, reports it at that level, and `ERROR` would leave both
/// silent — which is the condition a user would report as a bug.
pub(crate) const fn startup_filter(mode: DiagMode, verbose: bool) -> LevelFilter {
    if mode.is_json() {
        LevelFilter::OFF
    } else if verbose {
        LevelFilter::TRACE
    } else {
        LevelFilter::WARN
    }
}

/// Resolve and install the localizer used while parsing startup arguments.
pub(crate) fn startup_localizer(
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
pub(crate) struct StartupParse<'a> {
    /// Raw process arguments, including the program name.
    pub(crate) args: Vec<OsString>,
    /// Localizer built from the startup locale, before configuration merging.
    pub(crate) localizer: &'a Arc<dyn Localizer>,
    /// Diagnostic mode assumed before configuration can override it.
    pub(crate) mode: DiagMode,
    /// Colour choice handed to clap for help and version rendering.
    pub(crate) color_choice: ColorChoice,
    /// Buffer holding startup diagnostics until a path decides their stream.
    pub(crate) startup_writer: &'a StartupWriter,
}

/// Parse localized arguments, emitting valid JSON diagnostics when JSON is
/// selected.
///
/// Help and version requests exit through clap; all other parse failures return
/// the corresponding failure [`ExitCode`] on the JSON path.
pub(crate) fn parse_cli_or_exit(
    parse: StartupParse<'_>,
) -> Result<(cli::Cli, ArgMatches), ExitCode> {
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

/// Return whether clap's help or version renderer was requested in raw argv.
///
/// Only `--help` and `--version` reach clap's own renderer; the `help`
/// subcommand builds its command separately and takes its colour from the
/// resolved display plan instead.
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

#[cfg(test)]
mod tests;
