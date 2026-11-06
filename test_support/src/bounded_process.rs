//! Run a child process to completion within a deadline.
//!
//! Tests that start a compiler of their own would otherwise wait on it for as
//! long as the runner lets them. A hung `rustc` then looks like a slow test, and
//! only nextest's terminate-after reaps it, leaving the child behind. These
//! helpers kill the child at the deadline and fail with a message that names it.

use std::{
    io::{self, Read},
    process::{Child, Command, ExitStatus, Output, Stdio},
    sync::mpsc::{self, Receiver, RecvTimeoutError},
    time::{Duration, Instant},
};

/// How long a single-file compile may take before the test fails.
///
/// A cold `rustc` start on a loaded host takes seconds. This is well above that
/// and well below the profile's terminate-after allowance, so a hung compiler
/// fails the test with a message.
pub const COMPILE_DEADLINE: Duration = Duration::from_secs(120);

/// How long a compiled fixture may run before the test fails.
///
/// The fixtures print a verdict and exit; this only has to be longer than that.
pub const RUN_DEADLINE: Duration = Duration::from_secs(60);

/// How often a running child is checked against its deadline.
const POLL_INTERVAL: Duration = Duration::from_millis(25);

/// Run `command` and collect its output, killing it when it outlives `deadline`.
///
/// The deadline covers the child's whole run and the wait for its output. Both
/// pipes are drained on their own threads, so a chatty child cannot fill a pipe
/// and stall while this waits. The scope is the direct child: a grandchild that
/// keeps a pipe open after the child exits makes the wait for output run to the
/// deadline and fail, but is not itself killed, because no portable way to
/// signal a process group is available to test code.
///
/// # Errors
///
/// Returns an error when the child cannot be started or waited on, when a
/// reader thread panics, or, with kind [`io::ErrorKind::TimedOut`], when the
/// child is still running at the deadline (it is killed first) or its output is
/// still open at the deadline.
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
    let started = Instant::now();
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());

    let status = wait_within(&mut child, started, deadline)?;

    Ok(Output {
        status,
        stdout: collect(&stdout, started, deadline)?,
        stderr: collect(&stderr, started, deadline)?,
    })
}

/// Wait for `child` to exit, killing and reaping it at the deadline.
fn wait_within(child: &mut Child, started: Instant, deadline: Duration) -> io::Result<ExitStatus> {
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if started.elapsed() >= deadline {
            child.kill()?;
            child.wait()?;
            return Err(timed_out(deadline));
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

/// The error for a child, or its output, still open at the deadline.
fn timed_out(deadline: Duration) -> io::Error {
    io::Error::new(
        io::ErrorKind::TimedOut,
        format!("the child did not finish within {deadline:?}"),
    )
}

/// Read a child pipe to its end on a thread of its own.
fn drain<R: Read + Send + 'static>(pipe: Option<R>) -> Receiver<Vec<u8>> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut reader) = pipe {
            // What was read is all the diagnostics there are; a read error ends it.
            reader.read_to_end(&mut bytes).ok();
        }
        sender.send(bytes).ok();
    });
    receiver
}

/// Take a drained pipe's bytes within what remains of the deadline.
fn collect(
    reader: &Receiver<Vec<u8>>,
    started: Instant,
    deadline: Duration,
) -> io::Result<Vec<u8>> {
    reader
        .recv_timeout(deadline.saturating_sub(started.elapsed()))
        .map_err(|error| match error {
            RecvTimeoutError::Timeout => timed_out(deadline),
            RecvTimeoutError::Disconnected => io::Error::other("a pipe reader thread panicked"),
        })
}

#[cfg(all(test, unix))]
mod tests {
    //! A child past its deadline is killed; one inside it keeps its output.

    use super::*;

    fn shell(script: &str, arguments: &[&str]) -> Command {
        let mut command = Command::new("sh");
        command.arg("-c").arg(script).arg("sh").args(arguments);
        command
    }

    #[test]
    fn a_child_that_outlives_its_deadline_is_killed_and_reported() {
        let temp = tempfile::tempdir().expect("temp dir");
        let pid_file = temp.path().join("pid");
        let mut command = shell(
            "echo $$ > \"$1\"; exec sleep 30",
            &[pid_file.to_str().expect("UTF-8 path")],
        );

        let started = Instant::now();
        let outcome = output_within(&mut command, Duration::from_millis(400));

        assert!(
            matches!(&outcome, Err(error) if error.kind() == io::ErrorKind::TimedOut),
            "a child past its deadline must fail with TimedOut, got {outcome:?}",
        );
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "the wait must end at the deadline"
        );
        let pid = cap_std::fs::Dir::open_ambient_dir(temp.path(), cap_std::ambient_authority())
            .and_then(|dir| dir.read_to_string("pid"))
            .expect("the child recorded its pid");
        let alive = Command::new("kill")
            .args(["-0", pid.trim()])
            .stderr(Stdio::null())
            .status()
            .expect("kill is available")
            .success();
        assert!(
            !alive,
            "process {} must have been killed and reaped",
            pid.trim()
        );
    }

    #[test]
    fn a_child_inside_its_deadline_keeps_its_status_and_both_streams() {
        let mut command = shell("echo out; echo diagnostic >&2; exit 3", &[]);

        let output = output_within(&mut command, Duration::from_secs(20))
            .expect("a quick child finishes inside its deadline");

        assert_eq!(output.status.code(), Some(3));
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "out");
        assert_eq!(String::from_utf8_lossy(&output.stderr).trim(), "diagnostic");
    }

    #[test]
    fn output_larger_than_a_pipe_buffer_on_both_streams_is_kept_whole() {
        const SIZE: usize = 300_000;
        let mut command = shell(
            "head -c 300000 /dev/zero; head -c 300000 /dev/zero >&2",
            &[],
        );

        let output = output_within(&mut command, Duration::from_secs(20))
            .expect("a child writing past the pipe capacity still completes");

        assert!(output.status.success());
        assert_eq!(output.stdout.len(), SIZE);
        assert_eq!(output.stderr.len(), SIZE);
    }

    #[test]
    fn a_grandchild_holding_the_pipe_open_ends_at_the_deadline() {
        // The shell exits at once, but its background `sleep` keeps stdout open.
        let mut command = shell("sleep 3 & exit 0", &[]);

        let started = Instant::now();
        let outcome = output_within(&mut command, Duration::from_millis(400));

        assert!(
            matches!(&outcome, Err(error) if error.kind() == io::ErrorKind::TimedOut),
            "output still open at the deadline must fail with TimedOut, got {outcome:?}",
        );
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "the wait must not outlast the grandchild"
        );
    }
}
