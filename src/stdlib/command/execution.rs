//! Shell execution helpers shared by `shell` and `grep` filters.

use std::{
    io::{self, Write},
    process::{Child, Command, ExitStatus, Stdio},
    sync::Arc,
    thread,
    time::Duration,
};

use super::{
    config::{OutputMode, OutputStream, PipeSpec},
    context::CommandContext,
    error::CommandFailure,
    pipes::{cleanup_readers, handle_stdin_result, join_reader, spawn_pipe_reader},
    result::{PipeOutcome, StdoutResult},
};
use wait_timeout::ChildExt;

#[cfg(windows)]
pub(super) const SHELL: &str = "cmd";
#[cfg(windows)]
pub(super) const SHELL_ARGS: &[&str] = &["/C"];

#[cfg(not(windows))]
pub(super) const SHELL: &str = "sh";
#[cfg(not(windows))]
pub(super) const SHELL_ARGS: &[&str] = &["-c"];

pub(super) const COMMAND_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) fn run_command(
    command: &str,
    input: &[u8],
    context: &CommandContext,
) -> Result<StdoutResult, CommandFailure> {
    let mut cmd = Command::new(SHELL);
    cmd.args(SHELL_ARGS)
        .arg(command)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    run_child(cmd, input, context)
}

#[cfg(windows)]
pub(super) fn run_program(
    program: &str,
    args: &[String],
    input: &[u8],
    context: &CommandContext,
) -> Result<StdoutResult, CommandFailure> {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    run_child(cmd, input, context)
}

fn run_child(
    mut command: Command,
    input: &[u8],
    context: &CommandContext,
) -> Result<StdoutResult, CommandFailure> {
    let mut child = command.spawn().map_err(CommandFailure::Spawn)?;
    let mut stdin_handle = child.stdin.take().map(|mut stdin| {
        let buffer = input.to_vec();
        thread::spawn(move || stdin.write_all(&buffer))
    });

    let stdout_limit = match context.stdout_mode() {
        OutputMode::Capture => context.config().max_capture_bytes,
        OutputMode::Tempfile => context.config().max_stream_bytes,
    };
    let stderr_limit = context.config().max_capture_bytes;

    let stdout_spec = PipeSpec::new(OutputStream::Stdout, context.stdout_mode(), stdout_limit);
    let stderr_spec = PipeSpec::new(OutputStream::Stderr, OutputMode::Capture, stderr_limit);

    let stdout_config = context.config_handle();
    let stderr_config = context.config_handle();

    let mut stdout_reader =
        spawn_pipe_reader(child.stdout.take(), stdout_spec, Arc::clone(&stdout_config));
    let mut stderr_reader =
        spawn_pipe_reader(child.stderr.take(), stderr_spec, Arc::clone(&stderr_config));

    let status = match wait_for_exit(&mut child, COMMAND_TIMEOUT) {
        Ok(status) => status,
        Err(err) => {
            cleanup_readers(&mut stdout_reader, &mut stderr_reader, &mut stdin_handle);
            return Err(err);
        }
    };

    let stdout = join_reader(stdout_reader.take(), stdout_spec, stdout_config.as_ref())?;
    let stderr_outcome = join_reader(stderr_reader.take(), stderr_spec, stderr_config.as_ref())?;

    let stderr = match stderr_outcome {
        PipeOutcome::Bytes(bytes) => bytes,
        PipeOutcome::Tempfile(path) => {
            tracing::warn!(?path, "stderr reader returned a temp file; discarding path");
            Vec::new()
        }
    };

    handle_stdin_result(stdin_handle.take(), status.code(), &stderr)?;

    if status.success() {
        Ok(match stdout {
            PipeOutcome::Bytes(bytes) => StdoutResult::Bytes(bytes),
            PipeOutcome::Tempfile(path) => StdoutResult::Tempfile(path),
        })
    } else {
        Err(CommandFailure::Exit {
            status: status.code(),
            stderr,
        })
    }
}

pub(super) fn wait_for_exit(
    child: &mut Child,
    timeout: Duration,
) -> Result<ExitStatus, CommandFailure> {
    if let Some(status) = child.wait_timeout(timeout).map_err(CommandFailure::Io)? {
        Ok(status)
    } else {
        if let Err(err) = child.kill()
            && err.kind() != io::ErrorKind::InvalidInput
        {
            return Err(CommandFailure::Io(err));
        }
        if let Err(err) = child.wait() {
            tracing::warn!("failed to reap timed-out command: {err}");
        }
        Err(CommandFailure::Timeout(timeout))
    }
}
