//! Process helpers for Ninja file lifecycle, argument redaction, and subprocess I/O.
//! Internal to `runner`; public API is defined in `runner.rs`.

use super::BuildTargets;
use command_list_targets::CommandListTargetIndex;
use monotony::{MonotonicClock, StdMonotonicClock};
use std::{io, process::Command};

use camino::Utf8Path;
mod child_exit;
pub(in crate::runner) mod command_list_targets;
mod list_failure_telemetry;
mod logging;

mod dyndep;
#[cfg(test)]
mod exit_status_tests;
mod failure_attribution;
mod file_io;
mod ninja;
mod output_forwarding;
mod paths;
mod redaction;
mod streaming;
#[cfg(test)]
mod test_fixtures;
#[cfg(test)]
mod tests;

use child_exit::{ExitFailureContext, check_exit_status_with_context};
pub use dyndep::MAX_RETAINED_DYNDEP_FILES;
pub(crate) use dyndep::{DyndepPublicationLease, materialize_dyndep_files, prune_dyndep_cache};
pub use file_io::*;
use logging::{CommandLogContext, command_span, log_command_execution, log_command_spawn_failure};
pub use ninja::resolve_ninja_program;
use output_forwarding::{StatusObserver, spawn_and_stream_output};
pub use streaming::NINJA_STATUS_OVERSIZED_LINES_TOTAL;

mod configure;
mod environment;
mod job_count;
mod request;
mod stderr_mode;
use configure::{configure_ninja_build_command, configure_ninja_tool_command};
pub use environment::CommandEnv;
pub use job_count::NinjaJobCount;
pub use paths::*;
pub use request::{NinjaBuildRequest, NinjaProcessOptions, NinjaToolRequest};
pub use stderr_mode::StderrMode;

/// Per-invocation process settings passed only from Ninja setup to execution.
struct CommandExecutionContext<'a, Clock> {
    /// Invocation description attached to logs and spans.
    operation: &'a str,
    /// Child stderr policy controlling forwarding and attribution.
    stderr_mode: StderrMode,
    /// Whether stdout forwarding should retain the bounded failure tail.
    captures_ninja_failure_output: bool,
    /// Monotonic clock measuring the child run.
    clock: &'a Clock,
    /// Invocation-local output-name lookup for command-list fingerprints.
    command_list_targets: Option<&'a CommandListTargetIndex>,
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
        create_temp_ninja_file, resolve_ninja_program, write_ninja_file, write_text_file_utf8,
    };
}

/// Spawn and stream a prepared command, converting a failed exit into an error.
///
/// # Errors
///
/// Returns an error when the command fails to spawn, a child standard stream
/// cannot be taken or drained, or the command reports a non-zero exit status.
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
        command_list_targets: execution.command_list_targets,
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
/// use camino::Utf8Path;
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
///     program: Utf8Path::new("ninja"),
///     options: &options,
///     build_file: Utf8Path::new("build.ninja"),
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

/// Run a Ninja build with an injected clock, without status observation.
///
/// # Errors
///
/// Returns an error when the Ninja process fails to spawn, the standard
/// streams are unavailable, or Ninja reports a non-zero exit status.
fn run_ninja_with_clock(
    request: &NinjaBuildRequest<'_>,
    clock: &impl MonotonicClock,
) -> io::Result<()> {
    run_ninja_build_internal(*request, None, clock)
}

/// Run a Ninja build with target attribution and optional progress observation.
///
/// # Errors
///
/// Returns an error when Ninja cannot be spawned or exits unsuccessfully.
pub(in crate::runner) fn run_ninja_with_command_list_targets(
    request: NinjaBuildRequest<'_>,
    status_observer: Option<StatusObserver<'_>>,
    command_list_targets: &CommandListTargetIndex,
) -> io::Result<()> {
    run_ninja_build_internal_with_targets(
        request,
        status_observer,
        Some(command_list_targets),
        &StdMonotonicClock,
    )
}

/// Invoke a Ninja tool with an explicit child-process environment.
///
/// # Examples
///
/// ```rust,no_run
/// use netsuke::runner::{
///     CommandEnv, NinjaProcessOptions, NinjaToolRequest, StderrMode, run_ninja_tool_with,
/// };
/// use camino::Utf8Path;
///
/// let options = NinjaProcessOptions::default();
/// run_ninja_tool_with(&NinjaToolRequest {
///     program: Utf8Path::new("ninja"),
///     options: &options,
///     build_file: Utf8Path::new("build.ninja"),
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

/// Identify a configured Ninja command and its diagnostic metadata.
enum NinjaInternalCommand<'request, 'targets> {
    /// Run a build and optionally resolve command-list fingerprints.
    Build {
        /// Build configuration passed to Ninja.
        request: NinjaBuildRequest<'request>,
        /// Invocation-local output-name lookup for command-list fingerprints.
        command_list_targets: Option<&'targets CommandListTargetIndex>,
    },
    /// Run a `ninja -t` tool command.
    Tool(NinjaToolRequest<'request>),
}

/// Process metadata derived from a build or tool command.
struct NinjaInternalSettings<'request, 'targets> {
    /// Ninja executable to invoke.
    program: &'request Utf8Path,
    /// Child stderr policy controlling forwarding and attribution.
    stderr_mode: StderrMode,
    /// Invocation description attached to logs and spans.
    operation: &'request str,
    /// Whether stdout forwarding should retain the bounded failure tail.
    captures_ninja_failure_output: bool,
    /// Invocation-local output-name lookup for command-list fingerprints.
    command_list_targets: Option<&'targets CommandListTargetIndex>,
}

impl NinjaInternalCommand<'_, '_> {
    /// Derive the process metadata appropriate to this command kind.
    const fn settings(&self) -> NinjaInternalSettings<'_, '_> {
        match self {
            Self::Build {
                request,
                command_list_targets,
            } => NinjaInternalSettings {
                program: request.program,
                stderr_mode: request.stderr_mode,
                operation: "build",
                captures_ninja_failure_output: true,
                command_list_targets: *command_list_targets,
            },
            Self::Tool(request) => NinjaInternalSettings {
                program: request.program,
                stderr_mode: request.stderr_mode,
                operation: request.tool,
                captures_ninja_failure_output: false,
                command_list_targets: None,
            },
        }
    }

    /// Configure the child command for this build or tool request.
    fn configure(&self, command: &mut Command) -> io::Result<()> {
        match self {
            Self::Build { request, .. } => configure_ninja_build_command(command, request),
            Self::Tool(request) => configure_ninja_tool_command(command, request),
        }
    }
}

/// Parameters retained while the shared Ninja process runner executes.
struct NinjaInternalRequest<'request, 'observer, 'targets> {
    /// Build or tool command and its diagnostic metadata source.
    command: NinjaInternalCommand<'request, 'targets>,
    /// Optional callback receiving parsed task-progress updates.
    status_observer: Option<StatusObserver<'observer>>,
}

/// Build, spawn, and stream a Ninja invocation through a shared core.
///
/// # Errors
///
/// Returns an error when command configuration fails, the Ninja process
/// cannot be spawned, a child standard stream is unavailable, or Ninja
/// reports a non-zero exit status.
fn run_ninja_internal<Clock>(
    request: NinjaInternalRequest<'_, '_, '_>,
    clock: &Clock,
) -> io::Result<()>
where
    Clock: MonotonicClock,
{
    let settings = request.command.settings();
    let mut cmd = Command::new(settings.program.as_std_path());
    request.command.configure(&mut cmd)?;
    let execution = CommandExecutionContext {
        operation: settings.operation,
        stderr_mode: settings.stderr_mode,
        captures_ninja_failure_output: settings.captures_ninja_failure_output,
        clock,
        command_list_targets: settings.command_list_targets,
    };
    run_command_and_stream_with_context(cmd, request.status_observer, &execution)
}
/// Run a Ninja build with an injected clock and optional status observation.
///
/// # Errors
///
/// Returns an error when the Ninja process fails to spawn or reports a non-zero
/// exit status.
fn run_ninja_build_internal(
    request: NinjaBuildRequest<'_>,
    status_observer: Option<StatusObserver<'_>>,
    clock: &impl MonotonicClock,
) -> io::Result<()> {
    run_ninja_build_internal_with_targets(request, status_observer, None, clock)
}

/// Run a Ninja build with optional progress observation and target attribution.
///
/// # Errors
///
/// Returns an error when Ninja cannot be spawned or exits unsuccessfully.
fn run_ninja_build_internal_with_targets(
    request: NinjaBuildRequest<'_>,
    status_observer: Option<StatusObserver<'_>>,
    command_list_targets: Option<&CommandListTargetIndex>,
    clock: &impl MonotonicClock,
) -> io::Result<()> {
    run_ninja_internal(
        NinjaInternalRequest {
            command: NinjaInternalCommand::Build {
                request,
                command_list_targets,
            },
            status_observer,
        },
        clock,
    )
}

/// Run a Ninja tool with an injected clock and optional status observation.
///
/// # Errors
///
/// Returns an error when the Ninja process fails to spawn or reports a non-zero
/// exit status.
fn run_ninja_tool_internal(
    request: NinjaToolRequest<'_>,
    status_observer: Option<StatusObserver<'_>>,
    clock: &impl MonotonicClock,
) -> io::Result<()> {
    run_ninja_internal(
        NinjaInternalRequest {
            command: NinjaInternalCommand::Tool(request),
            status_observer,
        },
        clock,
    )
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
