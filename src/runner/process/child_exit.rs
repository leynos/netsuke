//! Child-process shutdown and Ninja non-zero exit conversion helpers.

use monotony::MonotonicClock;
use std::{
    io,
    process::{Child, ExitStatus},
    thread,
    time::Instant,
};

use super::{
    command_list_telemetry,
    command_logging::{CommandLogContext, log_command_exit_failure},
    failure_attribution::CommandListFailure,
    streaming::ForwardStats,
};

/// Context retained until the child process has completed.
#[derive(Clone, Copy)]
pub(super) struct ExitFailureContext<'failure, 'clock, Clock> {
    pub(super) operation: &'failure str,
    pub(super) suppress_stderr: bool,
    pub(super) command_list_failure: Option<&'failure CommandListFailure>,
    pub(super) clock: &'clock Clock,
    pub(super) started_at: Instant,
}

/// Return a child-process failure after recording any bounded command-list context.
pub(super) fn check_exit_status_with_context<Clock: MonotonicClock>(
    status: ExitStatus,
    context: &CommandLogContext,
    failure_context: &ExitFailureContext<'_, '_, Clock>,
) -> io::Result<()> {
    if status.success() {
        Ok(())
    } else {
        tracing::Span::current().record("failure_category", "exit_status");
        log_command_exit_failure(
            context,
            failure_context.operation,
            failure_context.suppress_stderr,
            status,
        );
        if let Some(failure) = failure_context.command_list_failure {
            command_list_telemetry::record_failure(
                failure,
                failure_context
                    .clock
                    .now()
                    .duration_since(failure_context.started_at),
            );
        }
        ninja_exit_error(status, failure_context.command_list_failure)
    }
}

/// Terminate a partially configured child and reap it before returning an error.
pub(super) fn terminate_child(child: &mut Child, context: &str) {
    if let Err(error) = child.kill() {
        tracing::debug!("failed to kill child after {context}: {error}");
    }
    if let Err(error) = child.wait() {
        tracing::debug!("failed to reap child after {context}: {error}");
    }
}

/// Convert a Ninja exit status into an error with optional bounded attribution.
pub(super) fn ninja_exit_error(
    status: ExitStatus,
    command_list_failure: Option<&CommandListFailure>,
) -> io::Result<()> {
    let message = command_list_failure.map_or_else(
        || format!("ninja exited with {status}"),
        |failure| format!("ninja exited with {status}: {failure}"),
    );
    Err(io::Error::other(message))
}

/// Join stderr forwarding and surface the child's wait result.
pub(super) fn finalize_streaming(
    wait_result: io::Result<ExitStatus>,
    stdout_stats: ForwardStats,
    err_handle: thread::JoinHandle<(ForwardStats, Option<CommandListFailure>)>,
) -> io::Result<(ExitStatus, Option<CommandListFailure>)> {
    handle_forwarding_stats(stdout_stats, "stdout");
    let command_list_failure = match err_handle.join() {
        Ok((stats, context)) => {
            handle_forwarding_stats(stats, "stderr");
            context
        }
        Err(error) => {
            tracing::warn!("stderr forwarding thread panicked: {error:?}");
            None
        }
    };
    wait_result.map(|status| (status, command_list_failure))
}

fn handle_forwarding_stats(stats: ForwardStats, stream_name: &str) {
    if stats.write_failed {
        tracing::debug!("{stream_name} forwarding encountered closed pipe; output truncated");
    }
}
