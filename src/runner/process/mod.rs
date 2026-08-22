//! Process helpers for Ninja file lifecycle, argument redaction, and subprocess I/O.
//! Internal to `runner`; public API is defined in `runner.rs`.

use super::BuildTargets;
use monotony::{MonotonicClock, StdMonotonicClock};
use std::{io, path::Path, process::Command};

mod child_exit;
mod command_list_telemetry;
mod command_logging;

mod dyndep_files;
mod dyndep_retention;
mod dyndep_telemetry;
#[cfg(test)]
mod exit_status_tests;
mod failure_attribution;
mod file_io;
mod ninja_program;
mod ninja_status;
mod output_forwarding;
mod paths;
mod redaction;
mod streaming;
#[cfg(test)]
mod tests;

use child_exit::{ExitFailureContext, check_exit_status_with_context};
use command_logging::{
    CommandLogContext, command_span, log_command_execution, log_command_spawn_failure,
};
pub(crate) use dyndep_files::materialize_dyndep_files;
pub use dyndep_retention::MAX_RETAINED_DYNDEP_FILES;
pub(crate) use dyndep_retention::{DyndepPublicationLease, prune_dyndep_cache};
pub use file_io::*;
pub use ninja_program::resolve_ninja_program;
#[cfg(doctest)]
pub use ninja_program::resolve_ninja_program_utf8;
#[cfg(test)]
use ninja_program::{resolve_ninja_program_utf8_with, resolve_ninja_program_with};
use output_forwarding::{StatusObserver, spawn_and_stream_output};

mod command_env;
mod configure;
mod job_count;
mod request;
mod stderr_mode;
pub use command_env::CommandEnv;
use configure::{configure_ninja_build_command, configure_ninja_tool_command};
pub use job_count::NinjaJobCount;
pub use paths::*;
pub use request::{NinjaBuildRequest, NinjaProcessOptions, NinjaToolRequest};
pub use stderr_mode::StderrMode;

/// Per-invocation process settings passed only from Ninja setup to execution.
struct CommandExecutionContext<'a, Clock> {
    operation: &'a str,
    stderr_mode: StderrMode,
    captures_ninja_failure_output: bool,
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
    let span = command_span(&context, execution.operation, execution.stderr_mode);
    let _entered = span.enter();

    log_command_execution(&context, execution.operation, execution.stderr_mode);
    let started_at = execution.clock.now();
    let child = cmd.spawn().inspect_err(|err| {
        tracing::Span::current().record("failure_category", "spawn");
        log_command_spawn_failure(&context, execution.operation, execution.stderr_mode, err);
    })?;
    let (status, command_list_failure) = spawn_and_stream_output(
        child,
        status_observer,
        execution.stderr_mode,
        execution.captures_ninja_failure_output,
    )?;
    let failure_context = ExitFailureContext {
        operation: execution.operation,
        stderr_mode: execution.stderr_mode,
        command_list_failure: command_list_failure.as_ref(),
        clock: execution.clock,
        started_at,
    };
    check_exit_status_with_context(status, &context, &failure_context)
}

/// Invoke Ninja with an explicit child-process environment.
///
/// Unlike [`crate::runner::run_ninja`], the caller supplies the environment
/// applied to the spawned command. Tests use this to place a fake Ninja on the
/// child's `PATH` without mutating the parent process, which would race every
/// other test in the same binary.
///
/// # Examples
///
/// ```rust,no_run
/// use netsuke::runner::{
///     BuildTargets, CommandEnv, NinjaBuildRequest, NinjaProcessOptions, StderrMode,
///     run_ninja_with,
/// };
/// use std::path::Path;
///
/// let options = NinjaProcessOptions::default();
/// let targets = BuildTargets::default();
/// // `inherit()` reproduces `run_ninja`; `with_path` replaces the child's
/// // `PATH` outright rather than prepending, so compose the whole value
/// // first. Either way the parent process is untouched.
/// let path = std::env::join_paths(["/opt/toolchain/bin", "/usr/bin"])
///     .expect("separator-free entries always join");
/// let env = CommandEnv::inherit().with_path(&path);
/// run_ninja_with(&NinjaBuildRequest {
///     program: Path::new("ninja"),
///     options: &options,
///     build_file: Path::new("build.ninja"),
///     targets: &targets,
///     env: &env,
///     stderr_mode: StderrMode::Forward,
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

/// Invoke a Ninja tool with an explicit child-process environment.
///
/// # Examples
///
/// ```rust,no_run
/// use netsuke::runner::{
///     CommandEnv, NinjaProcessOptions, NinjaToolRequest, StderrMode, run_ninja_tool_with,
/// };
/// use std::path::Path;
///
/// let options = NinjaProcessOptions::default();
/// run_ninja_tool_with(&NinjaToolRequest {
///     program: Path::new("ninja"),
///     options: &options,
///     build_file: Path::new("build.ninja"),
///     tool: "clean",
///     env: &CommandEnv::inherit(),
///     stderr_mode: StderrMode::Forward,
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
    stderr_mode: StderrMode,
    status_observer: Option<StatusObserver<'observer>>,
    operation: &'request str,
    captures_ninja_failure_output: bool,
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
        stderr_mode: request.stderr_mode,
        captures_ninja_failure_output: request.captures_ninja_failure_output,
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
            stderr_mode: request.stderr_mode,
            status_observer,
            operation: "build",
            captures_ninja_failure_output: true,
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
            stderr_mode: request.stderr_mode,
            status_observer,
            operation: request.tool,
            captures_ninja_failure_output: false,
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

/// Namespace for generated dyndep sidecar files.
pub(super) const DYNDEP_DIR: &str = ".netsuke/dyndep";
