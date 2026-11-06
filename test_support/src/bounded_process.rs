//! Run a child process to completion within a deadline.
//!
//! Tests that start a compiler of their own would otherwise wait on it for as
//! long as the runner lets them. A hung `rustc` then looks like a slow test, and
//! only nextest's terminate-after reaps it, leaving the child behind. These
//! helpers kill the child at the deadline and fail with a message that names it.

use std::{
    io::{self, Read},
    process::{Command, Output, Stdio},
    time::{Duration, Instant},
};

/// How long a single-file compile may take before the test fails.
///
/// A cold `rustc` start on a loaded host takes seconds. This is well above that
/// and well below the profile's terminate-after allowance, so a hung compiler
/// fails the test with a message.
pub const COMPILE_DEADLINE: Duration = Duration::from_secs(120);

/// How often a running child is checked against its deadline.
const POLL_INTERVAL: Duration = Duration::from_millis(25);

/// Run `command` and collect its output, killing it when it outlives `deadline`.
///
/// Both output pipes are drained on their own threads, so a chatty child cannot
/// fill a pipe and stall while this waits.
///
/// # Errors
///
/// Returns an error when the child cannot be started or waited on, when a
/// reader thread panics, or, with kind [`io::ErrorKind::TimedOut`], when the
/// child is still running at the deadline (it is killed first).
///
/// # Examples
///
/// ```rust,no_run
/// # use std::{process::Command, time::Duration};
/// # use test_support::bounded_process::output_within;
/// let mut command = Command::new("sh");
/// command.args(["-c", "echo ready"]);
/// let output = output_within(&mut command, Duration::from_secs(60)).expect("sh runs");
/// assert!(output.status.success());
/// ```
pub fn output_within(command: &mut Command, deadline: Duration) -> io::Result<Output> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());

    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() >= deadline {
            child.kill()?;
            child.wait()?;
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("the child did not finish within {deadline:?}"),
            ));
        }
        std::thread::sleep(POLL_INTERVAL);
    };

    Ok(Output {
        status,
        stdout: join(stdout)?,
        stderr: join(stderr)?,
    })
}

/// Read a child pipe to its end on a thread of its own.
fn drain<R: Read + Send + 'static>(pipe: Option<R>) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut reader) = pipe {
            // What was read is all the diagnostics there are; a read error ends it.
            reader.read_to_end(&mut bytes).ok();
        }
        bytes
    })
}

/// Collect a drained pipe, reporting a panicked reader as an I/O error.
fn join(reader: std::thread::JoinHandle<Vec<u8>>) -> io::Result<Vec<u8>> {
    reader
        .join()
        .map_err(|_| io::Error::other("a pipe reader thread panicked"))
}

#[cfg(all(test, unix))]
mod tests {
    //! A child past its deadline is killed; one inside it keeps its output.

    use super::*;

    #[test]
    fn a_child_that_outlives_its_deadline_is_killed_and_reported() {
        let mut command = Command::new("sleep");
        command.arg("30");

        let started = Instant::now();
        let outcome = output_within(&mut command, Duration::from_millis(200));

        assert!(
            matches!(&outcome, Err(error) if error.kind() == io::ErrorKind::TimedOut),
            "a child past its deadline must fail with TimedOut, got {outcome:?}",
        );
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "the child must be killed, not waited for",
        );
    }

    #[test]
    fn a_child_inside_its_deadline_keeps_its_status_and_both_streams() {
        let mut command = Command::new("sh");
        command.args(["-c", "echo out; echo diagnostic >&2; exit 3"]);

        let output = output_within(&mut command, Duration::from_secs(20))
            .expect("a quick child finishes inside its deadline");

        assert_eq!(output.status.code(), Some(3));
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "out");
        assert_eq!(String::from_utf8_lossy(&output.stderr).trim(), "diagnostic");
    }
}
