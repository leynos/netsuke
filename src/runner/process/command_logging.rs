//! Structured tracing helpers for prepared Ninja subprocess commands.
//!
//! Informational events retain only stable fields for tools that consume
//! structured diagnostics, while debug events retain the human-readable,
//! redacted command line for operators.

use super::StderrMode;
use super::command_env::env_names_eq;
use super::redaction::{CommandArg, redact_sensitive_args};
use camino::Utf8PathBuf;
use std::{
    ffi::OsStr,
    io,
    path::PathBuf,
    process::{Command, ExitStatus},
};
use tracing::{debug, field, info, info_span, warn};

/// Prepared, redacted logging representation of a Ninja [`Command`].
///
/// Retains the displayable program, redacted command text, and argument count
/// so individual logging paths do not reconstruct command metadata
/// inconsistently.
pub(super) struct CommandLogContext {
    /// Displayable program name, lossy for non-UTF-8 paths.
    pub(super) program_display: String,
    /// Redacted command line, safe to log verbatim.
    redacted_command: String,
    /// Number of redacted arguments shown on the command line.
    arg_count: usize,
    /// Number of environment overrides applied to the child.
    env_override_count: usize,
    /// Whether `PATH` itself is among the overrides.
    is_path_overridden: bool,
}

/// Summarize the command's environment overrides without disclosing them.
///
/// Diagnosing a Ninja failure caused by an injected environment needs to know
/// that overrides were applied and whether `PATH` was among them. Names and
/// values may carry secrets, so only a bounded count and a `PATH` flag are
/// derived; neither is user-controlled cardinality in a log field.
///
/// The `PATH` test uses [`env_names_eq`], so the flag answers the question an
/// operator is actually asking — "was the variable the child resolves programs
/// from overridden?" — under the target's own naming rules. Folding case
/// unconditionally would flag a Unix variable merely named `Path`, which is an
/// unrelated variable there.
fn summarize_env_overrides(cmd: &Command) -> (usize, bool) {
    let mut count = 0usize;
    let mut is_path_overridden = false;
    for (key, _) in cmd.get_envs() {
        count += 1;
        is_path_overridden |= env_names_eq(key, OsStr::new("PATH"));
    }
    (count, is_path_overridden)
}

impl CommandLogContext {
    /// Derives the shared logging context from a prepared [`Command`].
    ///
    /// Converts UTF-8 program paths directly, falls back to a lossy display
    /// representation for non-UTF-8 paths, and redacts sensitive arguments
    /// before they are logged.
    pub(super) fn from_command(cmd: &Command) -> Self {
        let program_path = PathBuf::from(cmd.get_program());
        let program_display = match Utf8PathBuf::from_path_buf(program_path) {
            Ok(path) => path.into_string(),
            Err(path) => path.to_string_lossy().into_owned(),
        };
        let args: Vec<CommandArg> = cmd
            .get_args()
            .map(|a| CommandArg::new(a.to_string_lossy().into_owned()))
            .collect();
        let redacted_args = redact_sensitive_args(&args);
        let arg_count = redacted_args.len();
        let arg_strings: Vec<&str> = redacted_args.iter().map(CommandArg::as_str).collect();
        let redacted_command = format!("{} {}", program_display, arg_strings.join(" "));
        let (env_override_count, is_path_overridden) = summarize_env_overrides(cmd);

        Self {
            program_display,
            redacted_command,
            arg_count,
            env_override_count,
            is_path_overridden,
        }
    }
}

/// Records the structured event emitted immediately before spawning Ninja.
///
/// Includes the operation, executable, argument count, environment-override
/// summary, and stderr suppression metadata. A debug companion event retains
/// the redacted command line for verbose diagnostics without increasing the
/// informational event's cardinality.
pub(super) fn log_command_execution(
    context: &CommandLogContext,
    operation: &str,
    stderr_mode: StderrMode,
) {
    info!(
        operation,
        ninja_program = %context.program_display,
        arg_count = context.arg_count,
        env_override_count = context.env_override_count,
        path_overridden = context.is_path_overridden,
        suppress_stderr = stderr_mode.is_suppress(),
        "Executing Ninja subprocess",
    );
    debug!(
        operation,
        ninja_program = %context.program_display,
        suppress_stderr = stderr_mode.is_suppress(),
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
    stderr_mode: StderrMode,
    err: &io::Error,
) {
    warn!(
        operation,
        ninja_program = %context.program_display,
        env_override_count = context.env_override_count,
        path_overridden = context.is_path_overridden,
        suppress_stderr = stderr_mode.is_suppress(),
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
    stderr_mode: StderrMode,
    status: ExitStatus,
) {
    warn!(
        operation,
        ninja_program = %context.program_display,
        env_override_count = context.env_override_count,
        path_overridden = context.is_path_overridden,
        suppress_stderr = stderr_mode.is_suppress(),
        failure_category = "exit_status",
        %status,
        "Ninja command exited unsuccessfully",
    );
}

/// Creates the `ninja_subprocess` tracing span with stable invocation fields.
///
/// The initially empty `failure_category` field is populated only when the
/// subprocess fails. `env_override_count` and `path_overridden` summarize the
/// injected environment without naming any variable.
pub(super) fn command_span(
    context: &CommandLogContext,
    operation: &str,
    stderr_mode: StderrMode,
) -> tracing::Span {
    info_span!(
        "ninja_subprocess",
        operation,
        ninja_program = %context.program_display,
        arg_count = context.arg_count,
        env_override_count = context.env_override_count,
        path_overridden = context.is_path_overridden,
        suppress_stderr = stderr_mode.is_suppress(),
        failure_category = field::Empty,
    )
}

#[cfg(test)]
mod tests {
    //! Unit tests for command-log context construction.

    use super::*;
    use crate::runner::CommandEnv;
    use rstest::rstest;
    use tracing_subscriber::filter::LevelFilter;

    /// Build the command-log context used by logging tests.
    fn logging_context() -> CommandLogContext {
        CommandLogContext::from_command(&Command::new("ninja"))
    }

    /// Capture the informational execution event emitted for `operation`.
    fn captured_execution_event(operation: &str) -> String {
        crate::test_tracing_capture::with_test_subscriber(LevelFilter::INFO, |captured| {
            log_command_execution(&logging_context(), operation, StderrMode::Forward);
            let events = captured.snapshot();
            let [event] = events.as_slice() else {
                panic!("expected one command execution event, got {events:?}");
            };
            event.clone()
        })
    }

    /// The override summary counts overrides and flags `PATH` without naming
    /// or valuing any variable, so the fields stay safe to log.
    #[rstest]
    #[case(CommandEnv::inherit(), 0, false)]
    #[case(CommandEnv::inherit().with_var("NINJA_STATUS", "[%f/%t] "), 1, false)]
    #[case(CommandEnv::inherit().with_path("/opt/toolchain/bin"), 1, true)]
    #[case(
        CommandEnv::inherit()
            .with_var("NINJA_STATUS", "[%f/%t] ")
            .with_path("/opt/toolchain/bin"),
        2,
        true
    )]
    fn from_command_summarizes_env_overrides(
        #[case] env: CommandEnv,
        #[case] expected_count: usize,
        #[case] expected_path_overridden: bool,
    ) {
        let mut cmd = Command::new("ninja");
        env.apply(&mut cmd);

        let context = CommandLogContext::from_command(&cmd);

        assert_eq!(context.env_override_count, expected_count);
        assert_eq!(context.is_path_overridden, expected_path_overridden);
    }

    #[rstest]
    #[case::build("build")]
    #[case::named_tool("clean")]
    /// Verify informational execution logging retains only stable fields.
    fn execution_logging_preserves_stable_fields(#[case] operation: &str) {
        let event = captured_execution_event(operation);

        assert!(
            event.contains(&format!("operation={operation:?}")),
            "execution event should retain its operation label: {event}"
        );
        assert!(
            event.contains("ninja_program=ninja"),
            "execution event should retain its Ninja program: {event}"
        );
        assert!(
            event.contains("arg_count=0")
                && event.contains("env_override_count=0")
                && event.contains("path_overridden=false")
                && event.contains("suppress_stderr=false"),
            "execution event should retain stable command metadata: {event}"
        );
        assert!(
            event.contains("message=Executing Ninja subprocess"),
            "execution event should use its static informational message: {event}"
        );
        assert!(
            !event.contains("Executing command"),
            "execution event must not contain the redacted command: {event}"
        );
    }

    /// Verify debug logging carries the redacted command-line companion event.
    #[test]
    fn execution_logging_emits_redacted_command_at_debug_level() {
        let mut command = Command::new("ninja");
        command.arg("build.ninja");
        let context = CommandLogContext::from_command(&command);
        let events =
            crate::test_tracing_capture::with_test_subscriber(LevelFilter::DEBUG, |captured| {
                log_command_execution(&context, "build", StderrMode::Forward);
                captured.snapshot()
            });

        let Some(event) = events
            .iter()
            .find(|event| event.contains("message=Executing command:"))
        else {
            panic!("expected a debug command event, got {events:?}");
        };
        assert!(
            event.contains("Executing command: ninja build.ninja"),
            "debug event should contain the redacted command: {event}"
        );
        assert!(
            event.contains("operation=\"build\"")
                && event.contains("ninja_program=ninja")
                && event.contains("suppress_stderr=false"),
            "debug event should retain correlation fields: {event}"
        );
    }

    /// Build a failing Unix exit status for logging tests.
    #[cfg(unix)]
    fn failed_exit_status() -> ExitStatus {
        use std::os::unix::process::ExitStatusExt;

        ExitStatus::from_raw(1 << 8)
    }

    /// Build a failing Windows exit status for logging tests.
    #[cfg(windows)]
    fn failed_exit_status() -> ExitStatus {
        use std::os::windows::process::ExitStatusExt;

        ExitStatus::from_raw(1)
    }

    /// Verify exit failures record status diagnostics.
    #[test]
    fn exit_failure_logging_records_status_diagnostics() {
        let event =
            crate::test_tracing_capture::with_test_subscriber(LevelFilter::WARN, |captured| {
                log_command_exit_failure(
                    &logging_context(),
                    "clean",
                    StderrMode::Suppress,
                    failed_exit_status(),
                );
                let events = captured.snapshot();
                let [event] = events.as_slice() else {
                    panic!("expected one exit failure event, got {events:?}");
                };
                event.clone()
            });

        assert!(
            event.contains("operation=\"clean\""),
            "exit failure event should retain its operation label: {event}"
        );
        assert!(
            event.contains("failure_category=\"exit_status\""),
            "exit failure event should identify the failure category: {event}"
        );
        assert!(
            event.contains("status="),
            "exit failure event should include the process status: {event}"
        );
    }

    /// A Unix variable named `Path` is not `PATH`, and must not raise the flag.
    ///
    /// Kept separate from the table above because the expectation is
    /// target-specific: the same input is a genuine `PATH` override on Windows.
    #[cfg(unix)]
    #[test]
    fn mixed_case_path_is_not_a_path_override_on_unix() {
        let mut cmd = Command::new("ninja");
        CommandEnv::inherit()
            .with_var("Path", "/mixed/case")
            .apply(&mut cmd);

        let context = CommandLogContext::from_command(&cmd);

        assert_eq!(context.env_override_count, 1);
        assert!(
            !context.is_path_overridden,
            "a Unix variable named `Path` is not `PATH`"
        );
    }

    /// On Windows the same variable *is* `PATH`, so the flag must rise.
    #[cfg(windows)]
    #[test]
    fn mixed_case_path_is_a_path_override_on_windows() {
        let mut cmd = Command::new("ninja");
        CommandEnv::inherit()
            .with_var("Path", "C:\\mixed")
            .apply(&mut cmd);

        let context = CommandLogContext::from_command(&cmd);

        assert_eq!(context.env_override_count, 1);
        assert!(
            context.is_path_overridden,
            "Windows resolves `Path` and `PATH` to one variable"
        );
    }

    #[cfg(unix)]
    #[test]
    fn from_command_uses_lossy_display_for_non_utf8_program() {
        use std::os::unix::ffi::OsStringExt;

        let cmd = Command::new(std::ffi::OsString::from_vec(b"ninja-\xff".to_vec()));

        let context = CommandLogContext::from_command(&cmd);

        assert_eq!(context.program_display, "ninja-\u{fffd}");
    }
}
