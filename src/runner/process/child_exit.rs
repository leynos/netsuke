//! Child-process shutdown and Ninja non-zero exit conversion helpers.

use monotony::MonotonicClock;
use std::{
    io,
    process::{Child, ExitStatus},
    thread,
    time::Instant,
};

use super::{
    StderrMode,
    failure_attribution::CommandListFailure,
    list_failure_telemetry,
    logging::{CommandLogContext, log_command_exit_failure},
    streaming::ForwardStats,
};

/// Keep the bounded child-shutdown emitters below the module line cap.
///
/// The `#[path]` attribute is required rather than incidental: a plain
/// `mod emitters;` would need `child_exit/mod.rs`, which denies
/// `clippy::self_named_module_files` beside this non-`mod.rs` parent, while
/// naming the file `child_exit_emitters.rs` beside this one forms a shared
/// `child_` prefix that the module-layout contract rejects. Pointing `#[path]`
/// into a same-stem directory satisfies both, and leaves the module's path —
/// and so every `super::` reference — unchanged.
#[path = "child_exit/emitters.rs"]
mod emitters;

use emitters::{
    debug_child_kill_failed, debug_child_reap_failed, debug_forwarding_stream_truncated,
    warn_forwarding_thread_panicked,
};

/// Context retained until the child process has completed.
#[derive(Clone, Copy)]
pub(super) struct ExitFailureContext<'failure, 'clock, Clock> {
    /// Invocation description used in exit-failure logs.
    pub(super) operation: &'failure str,
    /// Child stderr policy, reflected in failure diagnostics.
    pub(super) stderr_mode: StderrMode,
    /// Bounded command-list attribution recovered from child output.
    pub(super) command_list_failure: Option<&'failure CommandListFailure>,
    /// Monotonic clock used to measure the child run.
    pub(super) clock: &'clock Clock,
    /// Instant the child was spawned, for failure-duration telemetry.
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
            failure_context.stderr_mode,
            status,
        );
        if let Some(failure) = failure_context.command_list_failure {
            list_failure_telemetry::record_failure(
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
        debug_child_kill_failed(context, &error);
    }
    if let Err(error) = child.wait() {
        debug_child_reap_failed(context, &error);
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
            warn_forwarding_thread_panicked(&error);
            None
        }
    };
    wait_result.map(|status| (status, command_list_failure))
}

/// Log a truncation debug event when a forwarding stream hit a closed pipe.
fn handle_forwarding_stats(stats: ForwardStats, stream_name: &str) {
    if stats.write_failed {
        debug_forwarding_stream_truncated(stream_name);
    }
}
