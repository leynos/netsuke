//! Structured tracing helpers for prepared Ninja subprocess commands.
//!
//! The parent process still emits the established human-readable command line
//! for operators, while also attaching stable tracing fields for tools that
//! consume structured diagnostics.

use super::redaction::{CommandArg, redact_sensitive_args};
use camino::Utf8PathBuf;
use std::{
    io,
    path::PathBuf,
    process::{Command, ExitStatus},
};
use tracing::{field, info, info_span, warn};

/// Prepared, redacted logging representation of a Ninja [`Command`].
///
/// Retains the displayable program, redacted command text, and redacted
/// argument count so individual logging paths do not reconstruct command
/// metadata inconsistently.
pub(super) struct CommandLogContext {
    pub(super) program_display: String,
    redacted_command: String,
    redacted_arg_count: usize,
}

impl CommandLogContext {
    /// Derives the shared logging context from a prepared [`Command`].
    ///
    /// Converts UTF-8 program paths directly, falls back to a lossy display
    /// representation for non-UTF-8 paths, and redacts sensitive arguments
    /// before they are logged.
    pub(super) fn from_command(cmd: &Command) -> Self {
        let program_path = PathBuf::from(cmd.get_program());
        let program_display = Utf8PathBuf::from_path_buf(program_path).map_or_else(
            |path| path.to_string_lossy().into_owned(),
            Utf8PathBuf::into_string,
        );
        let args: Vec<CommandArg> = cmd
            .get_args()
            .map(|a| CommandArg::new(a.to_string_lossy().into_owned()))
            .collect();
        let redacted_args = redact_sensitive_args(&args);
        let redacted_arg_count = redacted_args.len();
        let arg_strings: Vec<&str> = redacted_args.iter().map(CommandArg::as_str).collect();
        let redacted_command = format!("{} {}", program_display, arg_strings.join(" "));

        Self {
            program_display,
            redacted_command,
            redacted_arg_count,
        }
    }
}

/// Records the structured event emitted immediately before spawning Ninja.
///
/// Includes the operation, executable, redacted argument count, and stderr
/// suppression metadata.
pub(super) fn log_command_execution(
    context: &CommandLogContext,
    operation: &str,
    suppress_stderr: bool,
) {
    info!(
        operation,
        ninja_program = %context.program_display,
        redacted_arg_count = context.redacted_arg_count,
        suppress_stderr,
        "Executing command: {}",
        context.redacted_command,
    );
}

/// Records a structured warning when spawning the Ninja subprocess fails.
///
/// The associated subprocess span records the `"spawn"` failure category.
pub(super) fn log_command_spawn_failure(
    context: &CommandLogContext,
    operation: &str,
    suppress_stderr: bool,
    err: &io::Error,
) {
    warn!(
        operation,
        ninja_program = %context.program_display,
        suppress_stderr,
        failure_category = "spawn",
        error.kind = ?err.kind(),
        error = %err,
        "Ninja command failed to spawn",
    );
}

/// Records a structured warning for a non-successful Ninja child exit.
///
/// The associated subprocess span records the `"exit_status"` failure
/// category.
pub(super) fn log_command_exit_failure(
    context: &CommandLogContext,
    operation: &str,
    suppress_stderr: bool,
    status: ExitStatus,
) {
    warn!(
        operation,
        ninja_program = %context.program_display,
        suppress_stderr,
        failure_category = "exit_status",
        %status,
        "Ninja command exited unsuccessfully",
    );
}

/// Creates the `ninja_subprocess` tracing span with stable invocation fields.
///
/// The initially empty `failure_category` field is populated only when the
/// subprocess fails.
pub(super) fn command_span(
    context: &CommandLogContext,
    operation: &str,
    suppress_stderr: bool,
) -> tracing::Span {
    info_span!(
        "ninja_subprocess",
        operation,
        ninja_program = %context.program_display,
        redacted_arg_count = context.redacted_arg_count,
        suppress_stderr,
        failure_category = field::Empty,
    )
}
