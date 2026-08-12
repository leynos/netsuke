//! Child-process shutdown and Ninja non-zero exit conversion helpers.

use std::{
    io,
    process::{Child, ExitStatus},
    thread,
};

use super::streaming::ForwardStats;

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
    command_list_failure: Option<&str>,
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
    err_handle: thread::JoinHandle<(ForwardStats, Option<String>)>,
) -> io::Result<(ExitStatus, Option<String>)> {
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
