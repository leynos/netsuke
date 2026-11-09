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
        // Named for its one job rather than `stdout`, which would shadow the
        // child's pipe handle above and read as if it were the same stream.
        let mut fallback_writer = io::stdout().lock();
        if let Err(error) = report_unwritable_stderr(stderr_stats, &mut fallback_writer) {
            tracing::debug!("failed to report an unwritable stderr on stdout: {error}");
        }
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
///
/// The destination is a parameter rather than an [`io::stdout`] lock taken
/// here, so the process boundary owns which stream receives the warning and
/// this helper's own write and flush failures are testable without a real
/// terminal. The caller decides what a failure means; the fallback cannot
/// fail the build, because the stream that would report it is the one already
/// known to be broken.
fn report_unwritable_stderr(stats: ForwardStats, out: &mut impl Write) -> io::Result<()> {
    let rejected = match stats.failure {
        Some(kind @ (ForwardFailureKind::Write | ForwardFailureKind::Flush)) => Some(kind),
        Some(ForwardFailureKind::Read | ForwardFailureKind::Drain) | None => None,
    };
    let Some(kind) = rejected else {
        return Ok(());
    };

    writeln!(
        out,
        concat!(
            "netsuke: warning: failed to forward child stderr ({}); ",
            "reporting on stdout because the stderr destination rejected the output",
        ),
        kind.label()
    )
    .and_then(|()| out.flush())
}

#[cfg(test)]
mod tests {
    //! Cover the fallback warning's stream selection and its own failure path.

    use super::{ForwardFailureKind, ForwardStats, report_unwritable_stderr};
    use rstest::rstest;
    use std::io::{self, Write};

    /// Build forwarding statistics carrying one failure classification.
    fn stats_with(failure: Option<ForwardFailureKind>) -> ForwardStats {
        ForwardStats {
            bytes_read: 0,
            bytes_written: 0,
            failure,
            drain_failed: false,
        }
    }

    /// Writer that records every byte it accepts.
    #[derive(Default)]
    struct RecordingWriter {
        /// Bytes accepted so far.
        written: Vec<u8>,
        /// Whether `flush` should report a rejection.
        fails_flush: bool,
    }

    impl Write for RecordingWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.written.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            if self.fails_flush {
                return Err(io::Error::other("flush rejected"));
            }
            Ok(())
        }
    }

    /// Writer whose every operation is rejected.
    struct RejectingWriter;

    impl Write for RejectingWriter {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("write rejected"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[rstest]
    #[case(ForwardFailureKind::Write)]
    #[case(ForwardFailureKind::Flush)]
    fn a_rejected_destination_is_reported_on_the_injected_writer(#[case] kind: ForwardFailureKind) {
        let mut out = RecordingWriter::default();
        report_unwritable_stderr(stats_with(Some(kind)), &mut out)
            .expect("a recording writer accepts every write");
        let text = String::from_utf8(out.written).expect("the warning is valid UTF-8");
        assert!(
            text.contains("failed to forward child stderr"),
            "the fallback must name the lost stream: {text}"
        );
        assert!(
            text.contains("reporting on stdout"),
            "the fallback must name its channel: {text}"
        );
        assert!(
            text.contains(kind.label()),
            "the fallback must carry the failure kind: {text}"
        );
    }

    #[rstest]
    #[case::read(Some(ForwardFailureKind::Read))]
    #[case::drain(Some(ForwardFailureKind::Drain))]
    #[case::none(None)]
    fn a_stream_that_did_not_reject_output_writes_nothing(
        #[case] failure: Option<ForwardFailureKind>,
    ) {
        let mut out = RecordingWriter::default();
        report_unwritable_stderr(stats_with(failure), &mut out)
            .expect("a recording writer accepts every write");
        assert!(out.written.is_empty(), "no warning may be written");
    }

    #[test]
    fn a_rejected_fallback_write_surfaces_to_the_caller() {
        let error = report_unwritable_stderr(
            stats_with(Some(ForwardFailureKind::Write)),
            &mut RejectingWriter,
        )
        .expect_err("rejected write must surface");
        assert!(
            error.to_string().contains("write rejected"),
            "the caller must see the write error: {error}"
        );
    }

    #[test]
    fn a_rejected_fallback_flush_surfaces_to_the_caller() {
        let mut out = RecordingWriter {
            fails_flush: true,
            ..RecordingWriter::default()
        };
        let error = report_unwritable_stderr(stats_with(Some(ForwardFailureKind::Flush)), &mut out)
            .expect_err("rejected flush must surface");
        assert!(
            error.to_string().contains("flush rejected"),
            "the caller must see the flush error: {error}"
        );
    }
}
