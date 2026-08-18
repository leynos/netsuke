//! Shell execution helpers shared by `shell` and `grep` filters.

#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::{
    io::{self, Write},
    process::{Child, Command, ExitStatus, Stdio},
    sync::{Arc, Once},
    thread,
    time::{Duration, Instant},
};

use super::{
    config::{OutputMode, OutputStream, PipeSpec},
    context::CommandContext,
    error::CommandFailure,
    pipes::{cleanup_readers, handle_stdin_result, join_reader, spawn_pipe_reader},
    result::{PipeOutcome, StdoutResult},
};
use metrics::{counter, describe_counter, describe_histogram, histogram};
use tracing::field;
use wait_timeout::ChildExt;

const COMMAND_EXECUTIONS_TOTAL: &str = "netsuke_stdlib_command_executions_total";
const COMMAND_EXECUTION_DURATION: &str = "netsuke_stdlib_command_execution_duration_seconds";

#[derive(Clone, Copy)]
enum CommandOperation {
    Shell,
    #[cfg(windows)]
    Program,
}

impl CommandOperation {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Shell => "shell",
            #[cfg(windows)]
            Self::Program => "program",
        }
    }
}

#[cfg(windows)]
pub(super) const SHELL: &str = "cmd";
#[cfg(windows)]
pub(super) const SHELL_ARGS: &[&str] = &["/C"];

#[cfg(not(windows))]
pub(super) const SHELL: &str = "sh";
#[cfg(not(windows))]
pub(super) const SHELL_ARGS: &[&str] = &["-c"];

pub(super) const COMMAND_TIMEOUT: Duration = Duration::from_secs(5);

/// Pipe the child's standard input, output, and error streams.
///
/// Every command spawned here streams its input and captures both output
/// streams, so this configuration is shared by all construction sites.
fn configure_piped_stdio(command: &mut Command) {
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
}

/// Everything a configured child needs beyond its own argument list.
///
/// Grouping these keeps [`run_configured_command`] to the program, the one
/// varying step, and this bundle.
#[derive(Clone, Copy)]
struct ChildInvocation<'a> {
    input: &'a [u8],
    context: &'a CommandContext,
    operation: CommandOperation,
}

/// Build and run a child process, leaving only argument choice to the caller.
///
/// Every entry point constructs a `Command`, applies its arguments, pipes the
/// standard streams, and hands the result to [`run_child`]. Only the second step
/// differs between them, so `configure_args` is the sole variation point.
fn run_configured_command(
    program: &str,
    configure_args: impl FnOnce(&mut Command),
    invocation: ChildInvocation<'_>,
) -> Result<StdoutResult, CommandFailure> {
    let mut cmd = Command::new(program);
    configure_args(&mut cmd);
    configure_piped_stdio(&mut cmd);

    run_child(
        cmd,
        invocation.input,
        invocation.context,
        invocation.operation,
    )
}

pub(super) fn run_command(
    command: &str,
    input: &[u8],
    context: &CommandContext,
) -> Result<StdoutResult, CommandFailure> {
    run_configured_command(
        SHELL,
        |cmd| {
            configure_shell_command(cmd, command);
        },
        ChildInvocation {
            input,
            context,
            operation: CommandOperation::Shell,
        },
    )
}

/// Configure the host shell to execute one already-composed command string.
///
/// Windows `cmd.exe` parses its command argument itself, rather than using
/// the usual C-runtime argument rules. Passing a quoted command through
/// [`Command::arg`] would escape its quotes into literal backslashes, so pass
/// the shell text verbatim inside `cmd /C`'s required outer quote pair.
fn configure_shell_command(cmd: &mut Command, command: &str) {
    #[cfg(windows)]
    {
        cmd.args(SHELL_ARGS).raw_arg(format!("\"{command}\""));
    }
    #[cfg(not(windows))]
    {
        cmd.args(SHELL_ARGS).arg(command);
    }
}

#[cfg(windows)]
pub(super) fn run_program(
    program: &str,
    args: &[String],
    input: &[u8],
    context: &CommandContext,
) -> Result<StdoutResult, CommandFailure> {
    run_configured_command(
        program,
        |cmd| {
            cmd.args(args);
        },
        ChildInvocation {
            input,
            context,
            operation: CommandOperation::Program,
        },
    )
}

fn run_child(
    command: Command,
    input: &[u8],
    context: &CommandContext,
    operation: CommandOperation,
) -> Result<StdoutResult, CommandFailure> {
    describe_metrics();
    let span = tracing::trace_span!(
        "stdlib.command.run",
        operation = operation.as_str(),
        has_path_override = context.config().has_command_path_override(),
        outcome = field::Empty,
        error_category = field::Empty,
    );
    let _guard = span.enter();
    let started = Instant::now();
    let result = run_child_inner(command, input, context);
    let outcome = match &result {
        Ok(_) => {
            span.record("outcome", "success");
            "success"
        }
        Err(error) => {
            span.record("outcome", "error");
            span.record("error_category", error.category());
            tracing::debug!(
                error_category = error.category(),
                "configured child process failed"
            );
            "error"
        }
    };
    counter!(
        COMMAND_EXECUTIONS_TOTAL,
        "operation" => operation.as_str(),
        "outcome" => outcome,
    )
    .increment(1);
    histogram!(
        COMMAND_EXECUTION_DURATION,
        "operation" => operation.as_str(),
    )
    .record(started.elapsed());
    result
}

fn describe_metrics() {
    static DESCRIBE: Once = Once::new();
    DESCRIBE.call_once(|| {
        describe_counter!(
            COMMAND_EXECUTIONS_TOTAL,
            "Counts configured child command outcomes by bounded operation and outcome."
        );
        describe_histogram!(
            COMMAND_EXECUTION_DURATION,
            "Measures configured child command execution duration in seconds by bounded operation."
        );
    });
}

fn run_child_inner(
    mut command: Command,
    input: &[u8],
    context: &CommandContext,
) -> Result<StdoutResult, CommandFailure> {
    context.config().configure_environment(&mut command);
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
