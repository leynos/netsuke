//! Error types and helpers for translating command failures into `MiniJinja`
//! errors.

use std::{io, time::Duration};

use minijinja::{Error, ErrorKind};

use super::{
    config::{OutputMode, OutputStream},
    context::CommandLocation,
};
use crate::localization::{self, keys};

/// Represents command execution failures that can be surfaced to `MiniJinja`
/// callers.
#[derive(Debug)]
pub(super) enum CommandFailure {
    /// The process could not be spawned (executable missing, permission
    /// failure, etc.).
    Spawn(io::Error),
    /// An I/O error occurred while interacting with the running process.
    Io(io::Error),
    /// The process closed stdin early while we were still writing input.
    BrokenPipe {
        /// Underlying OS error.
        source: io::Error,
        /// Exit status if the process reported one.
        status: Option<i32>,
        /// Captured stderr bytes.
        stderr: Vec<u8>,
    },
    /// The process exited with a non-zero status or was terminated by a signal.
    Exit {
        /// Exit status (`None` when terminated by a signal).
        status: Option<i32>,
        /// Captured stderr bytes.
        stderr: Vec<u8>,
    },
    /// The process produced more data than the configured byte budget allows.
    OutputLimit {
        /// Which pipe exceeded the budget.
        stream: OutputStream,
        /// Whether capture or streaming mode was active.
        mode: OutputMode,
        /// The configured byte ceiling that was exceeded.
        limit: u64,
    },
    /// The process failed to exit before the timeout elapsed.
    Timeout(Duration),
}

#[rustfmt::skip]
impl From<io::Error> for CommandFailure { fn from(err: io::Error) -> Self { Self::Io(err) } }

/// Translates a `CommandFailure` into a `MiniJinja` `Error`, decorating the
/// message with template and command context.
///
/// # Parameters
///
/// - `err`: the command failure to convert.
/// - `template`: name of the template invoking the helper.
/// - `command`: the command string being executed.
pub(super) fn command_error(err: CommandFailure, template: &str, command: &str) -> Error {
    let location = CommandLocation::new(template, command);
    match err {
        CommandFailure::Spawn(spawn) => spawn_error(location, &spawn),
        CommandFailure::Io(io_err) => io_error(location, &io_err),
        CommandFailure::BrokenPipe {
            source,
            status,
            stderr,
        } => broken_pipe_error(location, &source, ExitDetails::new(status, &stderr)),
        CommandFailure::Exit { status, stderr } => {
            exit_error(location, ExitDetails::new(status, &stderr))
        }
        CommandFailure::OutputLimit {
            stream,
            mode,
            limit,
        } => output_limit_error(location, LimitExceeded::new(stream, mode, limit)),
        CommandFailure::Timeout(duration) => timeout_error(location, duration),
    }
}

fn spawn_error(location: CommandLocation<'_>, err: &io::Error) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        localization::message(keys::COMMAND_SPAWN_FAILED)
            .with_arg("location", location.describe())
            .with_arg("details", err.to_string())
            .to_string(),
    )
}

fn io_error(location: CommandLocation<'_>, err: &io::Error) -> Error {
    let mut message = localization::message(keys::COMMAND_IO_FAILED)
        .with_arg("location", location.describe())
        .with_arg("details", err.to_string())
        .to_string();
    if err.kind() == io::ErrorKind::BrokenPipe {
        message.push_str(&localization::message(keys::COMMAND_CLOSED_INPUT_EARLY).to_string());
    }
    Error::new(ErrorKind::InvalidOperation, message)
}

fn broken_pipe_error(
    location: CommandLocation<'_>,
    err: &io::Error,
    details: ExitDetails<'_>,
) -> Error {
    let mut message = localization::message(keys::COMMAND_BROKEN_PIPE)
        .with_arg("location", location.describe())
        .with_arg("details", err.to_string())
        .to_string();
    append_exit_status(&mut message, details.status);
    append_stderr(&mut message, details.stderr);
    Error::new(ErrorKind::InvalidOperation, message)
}

fn exit_error(location: CommandLocation<'_>, details: ExitDetails<'_>) -> Error {
    let mut message = details.status.map_or_else(
        || {
            localization::message(keys::COMMAND_TERMINATED_BY_SIGNAL)
                .with_arg("location", location.describe())
                .to_string()
        },
        |code| {
            localization::message(keys::COMMAND_EXITED_WITH_STATUS)
                .with_arg("location", location.describe())
                .with_arg("status", code)
                .to_string()
        },
    );
    append_stderr(&mut message, details.stderr);
    Error::new(ErrorKind::InvalidOperation, message)
}

fn output_limit_error(location: CommandLocation<'_>, exceeded: LimitExceeded) -> Error {
    let stream_label = localization::message(exceeded.stream.label_key()).to_string();
    let mode_label = localization::message(exceeded.mode.label_key()).to_string();
    Error::new(
        ErrorKind::InvalidOperation,
        localization::message(keys::COMMAND_OUTPUT_LIMIT_EXCEEDED)
            .with_arg("location", location.describe())
            .with_arg("stream", stream_label)
            .with_arg("mode", mode_label)
            .with_arg("limit", exceeded.limit)
            .to_string(),
    )
}

fn timeout_error(location: CommandLocation<'_>, duration: Duration) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        localization::message(keys::COMMAND_TIMEOUT)
            .with_arg("location", location.describe())
            .with_arg("seconds", duration.as_secs_f64())
            .to_string(),
    )
}

fn append_exit_status(message: &mut String, status: Option<i32>) {
    if let Some(code) = status {
        let suffix = localization::message(keys::COMMAND_EXIT_STATUS_SUFFIX)
            .with_arg("status", code)
            .to_string();
        message.push_str(&suffix);
    } else {
        message.push_str(&localization::message(keys::COMMAND_SIGNAL_SUFFIX).to_string());
    }
}

fn append_stderr(message: &mut String, stderr: &[u8]) {
    let stderr_text = String::from_utf8_lossy(stderr);
    let trimmed = stderr_text.trim();
    if !trimmed.is_empty() {
        message.push_str(": ");
        message.push_str(trimmed);
    }
}

#[derive(Clone, Copy)]
struct ExitDetails<'a> {
    status: Option<i32>,
    stderr: &'a [u8],
}

impl<'a> ExitDetails<'a> {
    const fn new(status: Option<i32>, stderr: &'a [u8]) -> Self {
        Self { status, stderr }
    }
}

#[derive(Clone, Copy)]
struct LimitExceeded {
    stream: OutputStream,
    mode: OutputMode,
    limit: u64,
}

impl LimitExceeded {
    const fn new(stream: OutputStream, mode: OutputMode, limit: u64) -> Self {
        Self {
            stream,
            mode,
            limit,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::localization::{self, keys};

    #[test]
    fn spawn_errors_include_source() {
        let err = command_error(
            CommandFailure::Spawn(io::Error::new(io::ErrorKind::NotFound, "command not found")),
            "template.html",
            "missing_cmd",
        );
        assert_eq!(err.kind(), ErrorKind::InvalidOperation);
        let location = CommandLocation::new("template.html", "missing_cmd").describe();
        let expected = localization::message(keys::COMMAND_SPAWN_FAILED)
            .with_arg("location", location)
            .with_arg("details", "command not found")
            .to_string();
        assert_eq!(err.to_string(), format!("invalid operation: {expected}"));
    }

    #[test]
    fn io_errors_detect_broken_pipe() {
        let err = command_error(
            CommandFailure::Io(io::Error::new(io::ErrorKind::BrokenPipe, "pipe closed")),
            "template.html",
            "cat",
        );
        assert_eq!(err.kind(), ErrorKind::InvalidOperation);
        let expected = localization::message(keys::COMMAND_CLOSED_INPUT_EARLY).to_string();
        assert!(
            err.to_string().contains(&expected),
            "io error should mention closed input: {err}"
        );
    }

    #[test]
    fn broken_pipe_errors_include_exit_details() {
        let err = command_error(
            CommandFailure::BrokenPipe {
                source: io::Error::new(io::ErrorKind::BrokenPipe, "pipe error"),
                status: Some(1),
                stderr: b"error message".to_vec(),
            },
            "template.html",
            "grep",
        );
        let message = err.to_string();
        let location = CommandLocation::new("template.html", "grep").describe();
        let expected_prefix = localization::message(keys::COMMAND_BROKEN_PIPE)
            .with_arg("location", location)
            .with_arg("details", "pipe error")
            .to_string();
        let expected_message = format!("invalid operation: {expected_prefix}");
        let status_suffix = localization::message(keys::COMMAND_EXIT_STATUS_SUFFIX)
            .with_arg("status", 1)
            .to_string();
        assert!(message.starts_with(&expected_message));
        assert!(message.contains(&status_suffix));
        assert!(message.contains("error message"));
    }

    #[test]
    fn output_limit_errors_describe_constraint() {
        let err = command_error(
            CommandFailure::OutputLimit {
                stream: OutputStream::Stdout,
                mode: OutputMode::Capture,
                limit: 1024,
            },
            "template.html",
            "cat",
        );
        let location = CommandLocation::new("template.html", "cat").describe();
        let stream = localization::message(keys::COMMAND_OUTPUT_STREAM_STDOUT).to_string();
        let mode = localization::message(keys::COMMAND_OUTPUT_MODE_CAPTURE).to_string();
        let expected = localization::message(keys::COMMAND_OUTPUT_LIMIT_EXCEEDED)
            .with_arg("location", location)
            .with_arg("stream", stream)
            .with_arg("mode", mode)
            .with_arg("limit", 1024)
            .to_string();
        assert_eq!(err.to_string(), format!("invalid operation: {expected}"));
    }

    #[test]
    fn exit_errors_include_status_and_stderr() {
        let err = command_error(
            CommandFailure::Exit {
                status: Some(42),
                stderr: b"boom!".to_vec(),
            },
            "template.html",
            "echo",
        );
        assert_eq!(err.kind(), ErrorKind::InvalidOperation);
        let location = CommandLocation::new("template.html", "echo").describe();
        let expected = format!(
            "{}: boom!",
            localization::message(keys::COMMAND_EXITED_WITH_STATUS)
                .with_arg("location", location)
                .with_arg("status", 42)
        );
        assert_eq!(err.to_string(), format!("invalid operation: {expected}"));
    }

    #[test]
    fn timeout_errors_report_duration() {
        let err = command_error(
            CommandFailure::Timeout(Duration::from_secs(3)),
            "template.html",
            "sleep",
        );
        assert_eq!(err.kind(), ErrorKind::InvalidOperation);
        let location = CommandLocation::new("template.html", "sleep").describe();
        let expected = localization::message(keys::COMMAND_TIMEOUT)
            .with_arg("location", location)
            .with_arg("seconds", 3.0)
            .to_string();
        assert_eq!(err.to_string(), format!("invalid operation: {expected}"));
    }
}
