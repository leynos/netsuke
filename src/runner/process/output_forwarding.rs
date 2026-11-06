//! Forward Ninja output while preserving bounded command-list attribution.

use super::{
    StderrMode,
    child_exit::{finalize_streaming, terminate_child},
    failure_attribution::{
        CommandListFailure, NinjaFailureOutputTail, forward_stderr_with_attribution,
    },
    streaming::{
        ForwardFailureKind, ForwardStats, ForwardStream, forward_child_output,
        forward_child_output_with_ninja_status,
    },
};
use std::{
    io::{self, BufReader, Write},
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

/// Forward child stdout, parsing status updates and optionally the failure tail.
///
/// Returns forwarding statistics and any command-list failure marker recovered
/// from the bounded output tail.
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
                ForwardStream::Stdout,
            ),
            None => forward_child_output(
                BufReader::new(stdout),
                &mut tail_writer,
                ForwardStream::Stdout,
            ),
        };
        return (stats, tail_writer.into_failure());
    }

    let stats = match status_observer {
        Some(observer) => forward_child_output_with_ninja_status(
            BufReader::new(stdout),
            output,
            observer,
            ForwardStream::Stdout,
        ),
        None => forward_child_output(BufReader::new(stdout), output, ForwardStream::Stdout),
    };
    (stats, None)
}

/// Stream a Ninja child and return its exit status and bounded failure marker.
pub(super) fn spawn_and_stream_output(
    mut child: Child,
    status_observer: Option<StatusObserver<'_>>,
    stderr_mode: StderrMode,
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

    let err_handle = thread::spawn(move || match stderr_mode {
        // Avoid a long-lived stderr lock: status observers invoked while
        // draining stdout may emit task updates to stderr, and that path must
        // not block behind stderr forwarding. In JSON diagnostics mode we still
        // drain child stderr, but discard it to keep stderr machine-readable.
        StderrMode::Suppress => forward_stderr_with_attribution(BufReader::new(stderr), io::sink()),
        StderrMode::Forward => {
            forward_stderr_with_attribution(BufReader::new(stderr), io::stderr())
        }
    });

    // Intentionally drain stdout on the main thread when `status_observer` is
    // present so forwarding and callback-driven status updates keep a stable
    // ordering; moving this elsewhere can regress output timing/interleaving.
    let (stdout_stats, stdout_failure) = match stderr_mode {
        StderrMode::Suppress => {
            let mut output = io::sink();
            forward_stdout(
                stdout,
                &mut output,
                status_observer,
                captures_ninja_failure_output,
            )
        }
        StderrMode::Forward => {
            let mut output = io::stdout().lock();
            forward_stdout(
                stdout,
                &mut output,
                status_observer,
                captures_ninja_failure_output,
            )
        }
    };

    // Capture the wait result without `?` so the stderr forwarding thread is
    // joined on every exit path. Returning early on a `wait()` error would
    // otherwise detach the thread, leaking it and discarding its result.
    let wait_result = child.wait();
    let (status, stderr_failure, stderr_stats) =
        finalize_streaming(wait_result, stdout_stats, err_handle)?;
    if matches!(stderr_mode, StderrMode::Forward) {
        report_unwritable_stderr(stderr_stats);
    }
    let failure = if status.success() {
        stderr_failure
    } else {
        stderr_failure.or(stdout_failure)
    };
    Ok((status, failure))
}

/// Warn on stdout when the stderr destination itself rejected child output.
///
/// A forwarding warning normally travels on stderr, beside the output it
/// describes. When stderr is the destination that failed — a closed pipe, as
/// produced by `netsuke build 2>&-` — that warning cannot reach the user, and
/// dropping it would leave the loss entirely unobservable. The fallback is
/// written after the stderr forwarding thread has joined, so it cannot
/// interleave with forwarded output, and JSON mode never reaches it because
/// [`StderrMode::Suppress`] drains both child streams to [`io::sink`].
///
/// Only a rejected write or flush triggers the fallback: a failure to read the
/// child stream says nothing about the destination's health.
fn report_unwritable_stderr(stats: ForwardStats) {
    let rejected = match stats.failure {
        Some(kind @ (ForwardFailureKind::Write | ForwardFailureKind::Flush)) => Some(kind),
        Some(ForwardFailureKind::Read | ForwardFailureKind::Drain) | None => None,
    };
    let Some(kind) = rejected else {
        return;
    };

    let mut stdout = io::stdout().lock();
    let message = writeln!(
        stdout,
        concat!(
            "netsuke: warning: failed to forward child stderr ({}); ",
            "reporting on stdout because the stderr destination rejected the output",
        ),
        kind.label()
    )
    .and_then(|()| stdout.flush());
    if let Err(error) = message {
        tracing::debug!("failed to report an unwritable stderr on stdout: {error}");
    }
}
