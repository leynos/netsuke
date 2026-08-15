//! Forward Ninja output while preserving bounded command-list attribution.

use super::{
    child_exit::{finalize_streaming, terminate_child},
    failure_attribution::{
        CommandListFailure, NinjaFailureOutputTail, forward_stderr_with_attribution,
    },
    streaming::{ForwardStats, forward_child_output, forward_child_output_with_ninja_status},
};
use std::{
    io::{self, BufReader},
    process::{Child, ExitStatus},
    thread,
};

/// Callback contract for task-progress updates from parsed Ninja status lines.
///
/// Accepts `(current, total, description)` where `current` and `total` are
/// progress counters and `description` is a human-readable status string.
/// This alias appears in `pub(crate)` function signatures and borrows a mutable
/// callback for the call duration, so callers can retain state across updates.
pub(super) type StatusObserver<'a> = &'a mut dyn FnMut(u32, u32, &str);

fn forward_stdout<W>(
    stdout: impl io::Read,
    output: &mut W,
    status_observer: Option<StatusObserver<'_>>,
    captures_ninja_failure_output: bool,
) -> (ForwardStats, Option<CommandListFailure>)
where
    W: io::Write,
{
    if captures_ninja_failure_output {
        let mut tail_writer = NinjaFailureOutputTail::new(output);
        let stats = match status_observer {
            Some(observer) => forward_child_output_with_ninja_status(
                BufReader::new(stdout),
                &mut tail_writer,
                observer,
                "stdout",
            ),
            None => forward_child_output(BufReader::new(stdout), &mut tail_writer, "stdout"),
        };
        return (stats, tail_writer.into_failure());
    }

    let stats = match status_observer {
        Some(observer) => forward_child_output_with_ninja_status(
            BufReader::new(stdout),
            output,
            observer,
            "stdout",
        ),
        None => forward_child_output(BufReader::new(stdout), output, "stdout"),
    };
    (stats, None)
}

/// Stream a Ninja child and return its exit status and bounded failure marker.
pub(super) fn spawn_and_stream_output(
    mut child: Child,
    status_observer: Option<StatusObserver<'_>>,
    suppress_stderr: bool,
    captures_ninja_failure_output: bool,
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
        forward_stdout(
            stdout,
            &mut output,
            status_observer,
            captures_ninja_failure_output,
        )
    } else {
        let mut output = io::stdout().lock();
        forward_stdout(
            stdout,
            &mut output,
            status_observer,
            captures_ninja_failure_output,
        )
    };

    // Capture the wait result without `?` so the stderr forwarding thread is
    // joined on every exit path. Returning early on a `wait()` error would
    // otherwise detach the thread, leaking it and discarding its result.
    let wait_result = child.wait();
    let (status, stderr_failure) = finalize_streaming(wait_result, stdout_stats, err_handle)?;
    let failure = if status.success() {
        stderr_failure
    } else {
        stderr_failure.or(stdout_failure)
    };
    Ok((status, failure))
}
