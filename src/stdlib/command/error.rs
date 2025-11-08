//! Error types and helpers for translating command failures into `MiniJinja`
//! errors.

use std::{fmt::Write as _, io, time::Duration};

use minijinja::{Error, ErrorKind};

use super::{
    config::{OutputMode, OutputStream},
    context::CommandLocation,
};

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
        format!("failed to spawn {}: {err}", location.describe()),
    )
}

fn io_error(location: CommandLocation<'_>, err: &io::Error) -> Error {
    let mut message = format!("{} failed: {err}", location.describe());
    if err.kind() == io::ErrorKind::BrokenPipe {
        message.push_str(" (command closed input early)");
    }
    Error::new(ErrorKind::InvalidOperation, message)
}

fn broken_pipe_error(
    location: CommandLocation<'_>,
    err: &io::Error,
    details: ExitDetails<'_>,
) -> Error {
    let mut message = format!(
        "{} failed: {err} (command closed input early)",
        location.describe()
    );
    append_exit_status(&mut message, details.status);
    append_stderr(&mut message, details.stderr);
    Error::new(ErrorKind::InvalidOperation, message)
}

fn exit_error(location: CommandLocation<'_>, details: ExitDetails<'_>) -> Error {
    let mut message = details.status.map_or_else(
        || format!("{} terminated by signal", location.describe()),
        |code| format!("{} exited with status {code}", location.describe()),
    );
    append_stderr(&mut message, details.stderr);
    Error::new(ErrorKind::InvalidOperation, message)
}

fn output_limit_error(location: CommandLocation<'_>, exceeded: LimitExceeded) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        format!(
            "{} exceeded {stream} {mode} limit of {limit} bytes",
            location.describe(),
            stream = exceeded.stream.describe(),
            mode = exceeded.mode.describe(),
            limit = exceeded.limit,
        ),
    )
}

fn timeout_error(location: CommandLocation<'_>, duration: Duration) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        format!(
            "{} timed out after {}s",
            location.describe(),
            duration.as_secs_f64()
        ),
    )
}

fn append_exit_status(message: &mut String, status: Option<i32>) {
    if let Some(code) = status {
        write!(message, "; exited with status {code}").expect("write to String cannot fail");
    } else {
        message.push_str("; terminated by signal");
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

    #[test]
    fn spawn_errors_include_source() {
        let err = command_error(
            CommandFailure::Spawn(io::Error::new(io::ErrorKind::NotFound, "command not found")),
            "template.html",
            "missing_cmd",
        );
        assert_eq!(err.kind(), ErrorKind::InvalidOperation);
        let message = err.to_string();
        assert!(
            message.contains("failed to spawn"),
            "spawn error should explain failure: {message}"
        );
        assert!(
            message.contains("command not found"),
            "spawn error should include io::Error text: {message}"
        );
    }

    #[test]
    fn io_errors_detect_broken_pipe() {
        let err = command_error(
            CommandFailure::Io(io::Error::new(io::ErrorKind::BrokenPipe, "pipe closed")),
            "template.html",
            "cat",
        );
        assert_eq!(err.kind(), ErrorKind::InvalidOperation);
        assert!(
            err.to_string().contains("closed input early"),
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
        assert!(message.contains("closed input early"));
        assert!(message.contains("status 1"));
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
        let message = err.to_string();
        assert!(message.contains("exceeded"));
        assert!(message.contains("stdout"));
        assert!(message.contains("capture"));
        assert!(message.contains("1024"));
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
        let message = err.to_string();
        assert!(
            message.contains("status 42"),
            "exit error should mention status: {message}"
        );
        assert!(
            message.contains("boom!"),
            "exit error should include stderr: {message}"
        );
    }

    #[test]
    fn timeout_errors_report_duration() {
        let err = command_error(
            CommandFailure::Timeout(Duration::from_secs(3)),
            "template.html",
            "sleep",
        );
        assert_eq!(err.kind(), ErrorKind::InvalidOperation);
        assert!(
            err.to_string().contains("3s"),
            "timeout error should mention duration: {err}"
        );
    }
}
