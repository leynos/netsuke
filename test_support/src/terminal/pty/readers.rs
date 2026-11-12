//! Drain PTY and piped child output without blocking process management.
//!
//! Both streams have to be read while the child is still running, or a full
//! pipe buffer stops it before it can exit and the test deadlocks instead of
//! failing. Reading blocks, though, and the same thread has to notice when the
//! child is gone — so each stream is drained on its own thread and this module
//! joins them. [`super::process`] owns spawning, signalling and reaping; this
//! module never decides that the child should stop.

use super::process::{kill_process_group, stop_child};
use anyhow::{Context, Result, anyhow};
use nix::{errno::Errno, unistd::Pid};
use std::{
    io::{self, Read},
    os::fd::OwnedFd,
    process::Child,
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

/// Delay between reader completion checks.
const POLL_INTERVAL: Duration = Duration::from_millis(10);
/// Grace period for output readers after the direct child exits.
const READER_TIMEOUT: Duration = Duration::from_secs(5);
/// Final grace period after killing the child process group.
const READER_KILL_TIMEOUT: Duration = Duration::from_secs(2);

/// Distinguish PTY descriptors from ordinary child pipes while reading.
enum OutputStream {
    /// Read from a PTY master with `nix::unistd::read`.
    Terminal(OwnedFd),
    /// Read from the un-attached child output stream.
    Piped(Box<dyn Read + Send>),
}

/// Hold reader threads until both streams have reached end-of-file.
pub(super) struct OutputReaders {
    /// Reader attached to the PTY master.
    terminal: JoinHandle<io::Result<Vec<u8>>>,
    /// Reader attached to the optional piped output stream.
    piped: Option<JoinHandle<io::Result<Vec<u8>>>>,
}

/// Start readers for the PTY master and optional child pipe.
///
/// If either thread cannot start, stop the child and join any reader already
/// running so no setup error leaves a process behind.
///
/// # Examples
///
/// ```ignore
/// let readers = start_output_readers(&mut child, group, terminal, None)?;
/// ```
pub(super) fn start_output_readers(
    child: &mut Child,
    process_group: Pid,
    terminal: OwnedFd,
    piped: Option<Box<dyn Read + Send>>,
) -> Result<OutputReaders> {
    let terminal_reader = match spawn_reader(OutputStream::Terminal(terminal), true) {
        Ok(reader) => reader,
        Err(error) => {
            stop_child(child, process_group)
                .context("stop child after terminal reader setup failure")?;
            return Err(error).context("start pseudo-terminal reader");
        }
    };
    let piped_reader = spawn_piped_reader(child, process_group, piped, &terminal_reader)?;
    Ok(OutputReaders {
        terminal: terminal_reader,
        piped: piped_reader,
    })
}

/// Start the optional pipe reader and clean up the terminal reader on failure.
///
/// # Examples
///
/// ```ignore
/// let reader = spawn_piped_reader(&mut child, group, Some(pipe), &terminal)?;
/// ```
fn spawn_piped_reader(
    child: &mut Child,
    process_group: Pid,
    piped_stream: Option<Box<dyn Read + Send>>,
    terminal_reader: &JoinHandle<io::Result<Vec<u8>>>,
) -> Result<Option<JoinHandle<io::Result<Vec<u8>>>>> {
    let Some(output_pipe) = piped_stream else {
        return Ok(None);
    };
    match spawn_reader(OutputStream::Piped(output_pipe), false) {
        Ok(reader) => Ok(Some(reader)),
        Err(error) => {
            stop_child(child, process_group)
                .context("stop child after piped reader setup failure")?;
            wait_for_reader_after_stop(terminal_reader)?;
            Err(error).context("start piped output reader")
        }
    }
}

/// Start a reader thread that drains one child output stream to end-of-file.
///
/// # Examples
///
/// ```ignore
/// let reader = spawn_reader(OutputStream::Terminal(master), true)?;
/// assert!(!reader.is_finished());
/// ```
fn spawn_reader(
    stream: OutputStream,
    is_terminal: bool,
) -> io::Result<JoinHandle<io::Result<Vec<u8>>>> {
    thread::Builder::new()
        .name(if is_terminal {
            "terminal-output-reader".to_owned()
        } else {
            "piped-output-reader".to_owned()
        })
        .spawn(move || drain_stream(stream))
}

/// Drain a terminal or pipe stream into a byte vector.
///
/// # Examples
///
/// ```ignore
/// let bytes = drain_stream(OutputStream::Terminal(master))?;
/// assert!(!bytes.is_empty());
/// ```
fn drain_stream(mut stream: OutputStream) -> io::Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        match read_stream(&mut stream, &mut buffer) {
            Ok(0) => break,
            Ok(count) => output.extend_from_slice(buffer.get(..count).unwrap_or_default()),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) if matches!(&stream, OutputStream::Terminal(_)) && is_pty_eof(&error) => {
                break;
            }
            Err(error) => return Err(error),
        }
    }
    Ok(output)
}

/// Read from the chosen stream and convert Unix descriptor errors to I/O errors.
///
/// # Examples
///
/// ```ignore
/// let count = read_stream(&mut stream, &mut buffer)?;
/// ```
fn read_stream(stream: &mut OutputStream, buffer: &mut [u8]) -> io::Result<usize> {
    match stream {
        OutputStream::Terminal(descriptor) => nix::unistd::read(descriptor, buffer)
            .map_err(|error| io::Error::from_raw_os_error(error as i32)),
        OutputStream::Piped(reader) => reader.read(buffer),
    }
}

/// Return whether an I/O error is the Linux pseudo-terminal EOF marker.
///
/// # Examples
///
/// ```ignore
/// let error = io::Error::from_raw_os_error(Errno::EIO as i32);
/// assert!(is_pty_eof(&error));
/// ```
pub(super) fn is_pty_eof(error: &io::Error) -> bool {
    error.raw_os_error() == Some(Errno::EIO as i32)
}

/// Wait for both readers, killing descendants if they keep descriptors open.
///
/// # Examples
///
/// ```ignore
/// let (terminal, piped) = finish_readers(readers, process_group)?;
/// ```
pub(super) fn finish_readers(
    readers: OutputReaders,
    process_group: Pid,
) -> Result<(Vec<u8>, Option<Vec<u8>>)> {
    if !wait_for_readers(&readers, READER_TIMEOUT) {
        kill_process_group(process_group)?;
        if !wait_for_readers(&readers, READER_KILL_TIMEOUT) {
            return Err(anyhow!(
                "output readers remained blocked after process-group kill"
            ));
        }
    }

    let terminal = join_reader(readers.terminal).context("join terminal output reader")?;
    let piped = readers
        .piped
        .map(|reader| join_reader(reader).context("join piped output reader"))
        .transpose()?;
    Ok((terminal, piped))
}

/// Wait until every output reader exits or the supplied grace period expires.
///
/// # Examples
///
/// ```ignore
/// assert!(wait_for_readers(&readers, Duration::from_secs(1)));
/// ```
fn wait_for_readers(readers: &OutputReaders, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while !readers_are_finished(readers) {
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(POLL_INTERVAL);
    }
    true
}

/// Report whether both reader threads have completed.
///
/// # Examples
///
/// ```ignore
/// if readers_are_finished(&readers) { /* join them */ }
/// ```
fn readers_are_finished(readers: &OutputReaders) -> bool {
    readers.terminal.is_finished() && readers.piped.as_ref().is_none_or(JoinHandle::is_finished)
}

/// Join a reader after its completion has been observed.
///
/// # Examples
///
/// ```ignore
/// let bytes = join_reader(reader)?;
/// assert!(!bytes.is_empty());
/// ```
fn join_reader(reader: JoinHandle<io::Result<Vec<u8>>>) -> Result<Vec<u8>> {
    reader
        .join()
        .map_err(|_| anyhow!("output reader thread panicked"))?
        .context("read child output")
}

/// Wait for the terminal reader after stopping the child during setup failure.
///
/// # Examples
///
/// ```ignore
/// wait_for_reader_after_stop(&terminal_reader)?;
/// ```
fn wait_for_reader_after_stop(reader: &JoinHandle<io::Result<Vec<u8>>>) -> Result<()> {
    let deadline = Instant::now() + READER_KILL_TIMEOUT;
    while !reader.is_finished() {
        if Instant::now() >= deadline {
            return Err(anyhow!(
                "terminal reader remained blocked after child stopped"
            ));
        }
        thread::sleep(POLL_INTERVAL);
    }
    Ok(())
}
