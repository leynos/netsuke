//! Run Unix test commands with selected streams attached to a pseudo-terminal.

#[cfg(test)]
#[path = "../pty_tests.rs"]
mod tests;

mod process;
mod readers;

use anyhow::{Result, anyhow};
use nix::unistd::Pid;
use process::{SpawnedChild, spawn_child, stop_child, wait_for_child};
use readers::{finish_readers, start_output_readers};
use std::{
    process::{Child, ExitStatus},
    time::Duration,
};

/// Select which child output stream is attached to the pseudo-terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attach {
    /// Attach both standard output and standard error to the terminal.
    Both,
    /// Attach standard error to the terminal and pipe standard output.
    StderrOnly,
    /// Attach standard output to the terminal and pipe standard error.
    StdoutOnly,
}

/// Describe a command and its pseudo-terminal capture settings.
pub struct PtyRequest {
    /// Command to spawn after assigning its terminal streams.
    pub command: std::process::Command,
    /// Stream routing for the child process.
    pub attach: Attach,
    /// Maximum time to wait for the direct child to exit.
    pub timeout: Duration,
}

/// Hold the child status and bytes captured from its terminal and pipe.
#[derive(Debug)]
pub struct PtyOutput {
    /// Exit status of the direct child process.
    pub status: ExitStatus,
    /// Bytes read from the pseudo-terminal master.
    pub terminal: Vec<u8>,
    /// Bytes read from the un-attached output stream, or empty when both streams attach.
    pub piped: Vec<u8>,
}

/// Run a command with terminal output captured separately from its piped stream.
///
/// The child receives a 120-by-40 terminal and a new process group. The helper
/// drains both streams concurrently and kills the process group if the direct
/// child exceeds `timeout` or leaves output descriptors open after it exits.
/// The spawn lock only serializes calls through this helper.
///
/// # Examples
///
/// ```
/// use std::{process::Command, time::Duration};
/// use test_support::terminal::{Attach, PtyRequest, run_in_pty};
///
/// fn main() -> anyhow::Result<()> {
///     let mut command = Command::new("/bin/sh");
///     command.args(["-c", "test -t 1 && test -t 2 && printf ok"]);
///     let output = run_in_pty(PtyRequest {
///         command,
///         attach: Attach::Both,
///         timeout: Duration::from_secs(5),
///     })?;
///     assert!(output.status.success());
///     assert!(output.terminal.ends_with(b"ok"));
///     Ok(())
/// }
/// ```
///
/// # Errors
///
/// Returns an error if the pseudo-terminal cannot be opened, descriptors or
/// reader threads cannot be prepared, the child cannot be spawned or waited
/// for, or an output reader remains blocked after the process group is killed.
pub fn run_in_pty(request: PtyRequest) -> Result<PtyOutput> {
    let SpawnedChild {
        mut child,
        process_group,
        terminal,
        piped,
        timeout,
    } = spawn_child(request)?;
    let readers = start_output_readers(&mut child, process_group, terminal, piped)?;
    let (status, timed_out) = wait_for_child_or_stop(&mut child, process_group, timeout)?;
    let (terminal_bytes, piped_bytes) = finish_readers(readers, process_group)?;
    ensure_child_did_not_time_out(timed_out, timeout)?;

    Ok(PtyOutput {
        status,
        terminal: terminal_bytes,
        piped: piped_bytes.unwrap_or_default(),
    })
}

/// Stop the child after a wait error so its process group cannot outlive the test.
///
/// # Examples
///
/// ```ignore
/// let outcome = wait_for_child_or_stop(&mut child)?;
/// ```
fn wait_for_child_or_stop(
    child: &mut Child,
    process_group: Pid,
    timeout: Duration,
) -> Result<(ExitStatus, bool)> {
    match wait_for_child(child, process_group, timeout) {
        Ok(outcome) => Ok(outcome),
        // The wait failure is the diagnosis the caller needs; a cleanup
        // failure is attached to it rather than replacing it, otherwise a
        // timed-out child that also refused to die reports only the symptom of
        // the cleanup and hides why the test failed.
        Err(error) => match stop_child(child, process_group) {
            // The status of a child that is already being abandoned cannot
            // change the diagnosis, so the pattern discards it unnamed rather
            // than binding a value that would then need dropping.
            Ok(_) => Err(error),
            Err(cleanup) => Err(error.context(format!(
                "also failed to stop the pseudo-terminal child: {cleanup:#}"
            ))),
        },
    }
}

/// Return an error when the direct child exceeded its configured timeout.
///
/// # Examples
///
/// ```ignore
/// ensure_child_did_not_time_out(false, Duration::from_secs(5))?;
/// ```
fn ensure_child_did_not_time_out(timed_out: bool, timeout: Duration) -> Result<()> {
    if timed_out {
        Err(anyhow!("pseudo-terminal child exceeded {timeout:?}"))
    } else {
        Ok(())
    }
}
