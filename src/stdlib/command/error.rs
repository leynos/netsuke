//! Error types and helpers for translating command failures into Jinja errors.

use std::{io, time::Duration};

use minijinja::{Error, ErrorKind};

use super::{
    config::{OutputMode, OutputStream},
    context::CommandLocation,
};

#[derive(Debug)]
pub(super) enum CommandFailure {
    Spawn(io::Error),
    Io(io::Error),
    BrokenPipe {
        source: io::Error,
        status: Option<i32>,
        stderr: Vec<u8>,
    },
    Exit {
        status: Option<i32>,
        stderr: Vec<u8>,
    },
    OutputLimit {
        stream: OutputStream,
        mode: OutputMode,
        limit: u64,
    },
    Timeout(Duration),
}

impl From<io::Error> for CommandFailure {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

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
            duration.as_secs()
        ),
    )
}

fn append_exit_status(message: &mut String, status: Option<i32>) {
    if let Some(code) = status {
        message.push_str("; exited with status ");
        let code_text = code.to_string();
        message.push_str(&code_text);
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
