//! Process helpers for Ninja file lifecycle, argument redaction, and subprocess I/O.
//! Internal to `runner`; public API is defined in `runner.rs`.

use super::BuildTargets;
use crate::cli::Cli;
use monotony::{MonotonicClock, StdMonotonicClock};
use std::{
    io::{self, BufReader},
    path::Path,
    process::{Child, Command, ExitStatus},
    thread,
};

mod child_exit;
mod command_list_telemetry;
mod command_logging;
mod failure_attribution;
mod file_io;
mod ninja_program;
mod ninja_status;
mod paths;
mod redaction;
mod streaming;
#[cfg(test)]
mod tests;

use child_exit::{
    ExitFailureContext, check_exit_status_with_context, finalize_streaming, terminate_child,
};
use command_logging::{
    CommandLogContext, command_span, log_command_execution, log_command_spawn_failure,
};
use failure_attribution::{
    CommandListFailure, FailureAttributionWriter, forward_stderr_with_attribution,
};
pub use file_io::*;
pub use ninja_program::resolve_ninja_program;
#[cfg(doctest)]
pub use ninja_program::resolve_ninja_program_utf8;
#[cfg(test)]
use ninja_program::{resolve_ninja_program_utf8_with, resolve_ninja_program_with};

mod command_env;
mod configure;
mod request;
pub use command_env::CommandEnv;
use configure::{configure_ninja_build_command, configure_ninja_tool_command};
pub use paths::*;
pub use request::{NinjaBuildRequest, NinjaToolRequest};
use streaming::{ForwardStats, forward_child_output, forward_child_output_with_ninja_status};

/// Callback contract for task-progress updates from parsed Ninja status lines.
///
/// Accepts `(current, total, description)` where `current` and `total` are
/// progress counters and `description` is a human-readable status string.
/// This alias appears in `pub(crate)` function signatures and borrows a mutable
/// callback for the call duration, so callers can retain state across updates.
type StatusObserver<'a> = &'a mut dyn FnMut(u32, u32, &str);

/// Per-invocation process settings passed only from Ninja setup to execution.
struct CommandExecutionContext<'a, Clock> {
    operation: &'a str,
    suppress_stderr: bool,
    clock: &'a Clock,
}

// Public helpers for doctests only. This exposes internal helpers as a stable
// testing surface without exporting them in release builds.
#[cfg(doctest)]
pub mod doc {
    //! Re-exports of otherwise-private `process` items for doctests only.
    //!
    //! Doctests compile as a separate crate and cannot reach `pub(crate)` or
    //! private items in `process`, so this module surfaces the redaction
    //! helpers and a handful of Ninja-invocation functions under `cfg(doctest)`
    //! to give doc examples something to call without widening the crate's
    //! release-build API.
    pub use super::redaction::{
        CommandArg, is_sensitive_arg, redact_argument, redact_sensitive_args,
    };
    pub use super::{
        create_temp_ninja_file, resolve_ninja_program, resolve_ninja_program_utf8,
        write_ninja_file, write_text_file_utf8,
    };
}

fn run_command_and_stream_with_context<Clock: MonotonicClock>(
    mut cmd: Command,
    status_observer: Option<StatusObserver<'_>>,
    execution: &CommandExecutionContext<'_, Clock>,
) -> io::Result<()> {
    let context = CommandLogContext::from_command(&cmd);
    let span = command_span(&context, execution.operation, execution.suppress_stderr);
    let _entered = span.enter();

    log_command_execution(&context, execution.operation, execution.suppress_stderr);
    let started_at = execution.clock.now();
    let child = cmd.spawn().inspect_err(|err| {
        tracing::Span::current().record("failure_category", "spawn");
        log_command_spawn_failure(
            &context,
            execution.operation,
            execution.suppress_stderr,
            err,
        );
    })?;
    let (status, command_list_failure) =
        spawn_and_stream_output(child, status_observer, execution.suppress_stderr)?;
    let failure_context = ExitFailureContext {
        operation: execution.operation,
        suppress_stderr: execution.suppress_stderr,
        command_list_failure: command_list_failure.as_ref(),
        clock: execution.clock,
        started_at,
    };
    check_exit_status_with_context(status, &context, &failure_context)
}

/// Invoke the Ninja executable with the provided CLI settings.
///
/// The function forwards the job count and working directory to Ninja,
/// specifies the temporary build file, and streams its standard output and
/// error back to the user.
///
/// # Errors
///
/// Returns an [`io::Error`] if the Ninja process fails to spawn, the standard
/// streams are unavailable, or when Ninja reports a non-zero exit status.
pub fn run_ninja(
    program: &Path,
    cli: &Cli,
    build_file: &Path,
    targets: &BuildTargets<'_>,
) -> io::Result<()> {
    run_ninja_with(&NinjaBuildRequest {
        program,
        cli,
        build_file,
        targets,
        env: &CommandEnv::inherit(),
    })
}

/// Invoke Ninja with an explicit child-process environment.
///
/// Unlike [`run_ninja`], the caller supplies the environment applied to the
/// spawned command. Tests use this to place a fake Ninja on the child's `PATH`
/// without mutating the parent process, which would race every other test in
/// the same binary.
///
/// # Examples
///
/// ```rust,no_run
/// use netsuke::cli::Cli;
/// use netsuke::runner::{BuildTargets, CommandEnv, NinjaBuildRequest, run_ninja_with};
/// use std::path::Path;
///
/// let cli = Cli::default();
/// let targets = BuildTargets::default();
/// // `inherit()` reproduces `run_ninja`; `with_path` replaces the child's
/// // `PATH` outright rather than prepending, so compose the whole value
/// // first. Either way the parent process is untouched.
/// let path = std::env::join_paths(["/opt/toolchain/bin", "/usr/bin"])
///     .expect("separator-free entries always join");
/// let env = CommandEnv::inherit().with_path(&path);
/// run_ninja_with(&NinjaBuildRequest {
///     program: Path::new("ninja"),
///     cli: &cli,
///     build_file: Path::new("build.ninja"),
///     targets: &targets,
///     env: &env,
/// })?;
/// # Ok::<(), std::io::Error>(())
/// ```
///
/// # Errors
///
/// Returns an [`io::Error`] if the Ninja process fails to spawn, the standard
/// streams are unavailable, or when Ninja reports a non-zero exit status.
pub fn run_ninja_with(request: &NinjaBuildRequest<'_>) -> io::Result<()> {
    run_ninja_with_clock(request, &StdMonotonicClock)
}

fn run_ninja_with_clock(
    request: &NinjaBuildRequest<'_>,
    clock: &impl MonotonicClock,
) -> io::Result<()> {
    run_ninja_build_internal(*request, None, clock)
}

/// Invoke a Ninja tool (e.g., `ninja -t clean`) with the provided CLI settings.
///
/// The function forwards the job count and working directory to Ninja,
/// specifies the build file, and streams its standard output and error back to
/// the user.
///
/// # Errors
///
/// Returns an [`io::Error`] if the Ninja process fails to spawn, the standard
/// streams are unavailable, or when Ninja reports a non-zero exit status.
pub fn run_ninja_tool(program: &Path, cli: &Cli, build_file: &Path, tool: &str) -> io::Result<()> {
    run_ninja_tool_with(&NinjaToolRequest {
        program,
        cli,
        build_file,
        tool,
        env: &CommandEnv::inherit(),
    })
}

/// Invoke a Ninja tool with an explicit child-process environment.
///
/// # Examples
///
/// ```rust,no_run
/// use netsuke::cli::Cli;
/// use netsuke::runner::{CommandEnv, NinjaToolRequest, run_ninja_tool_with};
/// use std::path::Path;
///
/// let cli = Cli::default();
/// run_ninja_tool_with(&NinjaToolRequest {
///     program: Path::new("ninja"),
///     cli: &cli,
///     build_file: Path::new("build.ninja"),
///     tool: "clean",
///     env: &CommandEnv::inherit(),
/// })?;
/// # Ok::<(), std::io::Error>(())
/// ```
///
/// # Errors
///
/// Returns an [`io::Error`] if the Ninja process fails to spawn, the standard
/// streams are unavailable, or when Ninja reports a non-zero exit status.
pub fn run_ninja_tool_with(request: &NinjaToolRequest<'_>) -> io::Result<()> {
    run_ninja_tool_internal(*request, None, &StdMonotonicClock)
}

struct NinjaInternalRequest<'request, 'observer> {
    program: &'request Path,
    cli: &'request Cli,
    status_observer: Option<StatusObserver<'observer>>,
    operation: &'request str,
}

fn run_ninja_internal<F, Clock>(
    request: NinjaInternalRequest<'_, '_>,
    clock: &Clock,
    configure: F,
) -> io::Result<()>
where
    F: FnOnce(&mut Command) -> io::Result<()>,
    Clock: MonotonicClock,
{
    let mut cmd = Command::new(request.program);
    configure(&mut cmd)?;
    let execution = CommandExecutionContext {
        operation: request.operation,
        suppress_stderr: request.cli.json,
        clock,
    };
    run_command_and_stream_with_context(cmd, request.status_observer, &execution)
}
fn run_ninja_build_internal(
    request: NinjaBuildRequest<'_>,
    status_observer: Option<StatusObserver<'_>>,
    clock: &impl MonotonicClock,
) -> io::Result<()> {
    run_ninja_internal(
        NinjaInternalRequest {
            program: request.program,
            cli: request.cli,
            status_observer,
            operation: "build",
        },
        clock,
        |cmd| configure_ninja_build_command(cmd, &request),
    )
}

fn run_ninja_tool_internal(
    request: NinjaToolRequest<'_>,
    status_observer: Option<StatusObserver<'_>>,
    clock: &impl MonotonicClock,
) -> io::Result<()> {
    run_ninja_internal(
        NinjaInternalRequest {
            program: request.program,
            cli: request.cli,
            status_observer,
            operation: request.tool,
        },
        clock,
        |cmd| configure_ninja_tool_command(cmd, &request),
    )
}

/// Invoke `ninja` build and stream parsed task updates from status lines.
///
/// # Errors
///
/// Returns an [`io::Error`] if the Ninja process fails to spawn, the standard
/// streams are unavailable, or when Ninja reports a non-zero exit status.
pub(crate) fn run_ninja_with_status(
    request: NinjaBuildRequest<'_>,
    status_observer: StatusObserver<'_>,
) -> io::Result<()> {
    run_ninja_build_internal(request, Some(status_observer), &StdMonotonicClock)
}

/// Invoke `ninja -t` and stream parsed task updates from status lines.
///
/// # Errors
///
/// Returns an [`io::Error`] if the Ninja process fails to spawn, the standard
/// streams are unavailable, or when Ninja reports a non-zero exit status.
pub(crate) fn run_ninja_tool_with_status(
    request: NinjaToolRequest<'_>,
    status_observer: StatusObserver<'_>,
) -> io::Result<()> {
    run_ninja_tool_internal(request, Some(status_observer), &StdMonotonicClock)
}

fn forward_stdout(
    stdout: impl io::Read,
    output: &mut impl io::Write,
    status_observer: Option<StatusObserver<'_>>,
) -> (ForwardStats, Option<CommandListFailure>) {
    let mut attribution_writer = FailureAttributionWriter::new(output);
    let stats = match status_observer {
        Some(observer) => forward_child_output_with_ninja_status(
            BufReader::new(stdout),
            &mut attribution_writer,
            observer,
            "stdout",
        ),
        None => forward_child_output(BufReader::new(stdout), &mut attribution_writer, "stdout"),
    };
    (stats, attribution_writer.into_failure())
}
fn spawn_and_stream_output(
    mut child: Child,
    status_observer: Option<StatusObserver<'_>>,
    suppress_stderr: bool,
) -> io::Result<(ExitStatus, Option<CommandListFailure>)> {
    let Some(stdout) = child.stdout.take() else {
        terminate_child(&mut child, "stdout pipe unavailable");
        return Err(io::Error::other("child process missing stdout pipe"));
    };
    let Some(stderr) = child.stderr.take() else {
        terminate_child(&mut child, "stderr pipe unavailable");
        return Err(io::Error::other("child process missing stderr pipe"));
    };

    let err_handle = thread::spawn(move || {
        // Avoid a long-lived stderr lock: status observers invoked while
        // draining stdout may emit task updates to stderr, and that path must
        // not block behind stderr forwarding. In JSON diagnostics mode we still
        // drain child stderr, but discard it to keep stderr machine-readable.
        if suppress_stderr {
            forward_stderr_with_attribution(BufReader::new(stderr), io::sink())
        } else {
            forward_stderr_with_attribution(BufReader::new(stderr), io::stderr())
        }
    });

    // Intentionally drain stdout on the main thread when `status_observer` is
    // present so forwarding and callback-driven status updates keep a stable
    // ordering; moving this elsewhere can regress output timing/interleaving.
    let (stdout_stats, stdout_failure) = if suppress_stderr {
        let mut output = io::sink();
        forward_stdout(stdout, &mut output, status_observer)
    } else {
        let mut output = io::stdout().lock();
        forward_stdout(stdout, &mut output, status_observer)
    };

    // Capture the wait result without `?` so the stderr forwarding thread is
    // joined on every exit path. Returning early on a `wait()` error would
    // otherwise detach the thread, leaking it and discarding its result.
    let wait_result = child.wait();
    let (status, stderr_failure) = finalize_streaming(wait_result, stdout_stats, err_handle)?;
    Ok((status, stderr_failure.or(stdout_failure)))
}
