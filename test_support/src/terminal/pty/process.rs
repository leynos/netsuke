//! Spawn PTY children and manage their process groups.
//!
//! Everything here exists so a test can hang up on a real terminal without
//! leaving anything running: the child gets its own process group, and
//! [`kill_process_group`] signals the group rather than the child so a
//! grandchild cannot outlive the test. [`super::readers`] owns the other half
//! of the lifecycle — draining the two streams while this module supervises the
//! child — because a PTY whose master side is not drained will block the child
//! before it ever exits.

use super::{Attach, PtyRequest};
use anyhow::{Context, Result, anyhow};
use nix::{
    errno::Errno,
    fcntl::{FcntlArg, FdFlag, fcntl},
    pty::{Winsize, openpty},
    sys::signal::{Signal, killpg},
    unistd::Pid,
};
use std::{
    io::Read,
    os::{
        fd::{AsFd, OwnedFd},
        unix::process::CommandExt,
    },
    process::{Child, Command, ExitStatus, Stdio},
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};

/// Delay between non-blocking child completion checks.
const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Serialize PTY creation and spawning so helper descriptors cannot leak.
static PTY_SPAWN_LOCK: Mutex<()> = Mutex::new(());

/// Hold a spawned child and the resources its output readers need.
pub(super) struct SpawnedChild {
    /// Direct child process handle.
    pub child: Child,
    /// Process group created for the child.
    pub process_group: Pid,
    /// PTY master descriptor, retained until its reader starts.
    pub terminal: OwnedFd,
    /// Unattached output stream, when the request routes one to a pipe.
    pub piped: Option<Box<dyn Read + Send>>,
    /// Maximum time allowed for the direct child.
    pub timeout: Duration,
}

/// Open a PTY, attach the requested streams and spawn the child.
///
/// # Examples
///
/// ```ignore
/// let child = spawn_child(request)?;
/// ```
pub(super) fn spawn_child(request: PtyRequest) -> Result<SpawnedChild> {
    let PtyRequest {
        mut command,
        attach,
        timeout,
    } = request;
    let spawn_guard = PTY_SPAWN_LOCK
        .lock()
        .map_err(|_| anyhow!("pseudo-terminal spawn lock is poisoned"))?;
    let (master, slave) = open_terminal_pair()?;
    attach_streams(&mut command, attach, slave)?;
    command.stdin(Stdio::null());
    command.process_group(0);

    let mut child = command.spawn().context("spawn pseudo-terminal child")?;
    drop(command);
    drop(spawn_guard);
    let process_group = Pid::from_raw(child.id().cast_signed());
    let piped = match take_piped_stream(&mut child, attach) {
        Ok(piped) => piped,
        Err(error) => {
            stop_child(&mut child, process_group)
                .context("stop child after output pipe setup failure")?;
            return Err(error);
        }
    };

    Ok(SpawnedChild {
        child,
        process_group,
        terminal: master,
        piped,
        timeout,
    })
}

/// Open a 120-by-40 terminal and make both ends close on exec.
///
/// # Examples
///
/// ```ignore
/// let (master, slave) = open_terminal_pair()?;
/// ```
fn open_terminal_pair() -> Result<(OwnedFd, OwnedFd)> {
    let window = Winsize {
        ws_row: 40,
        ws_col: 120,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    let pair = openpty(Some(&window), None).context("open a pseudo-terminal")?;
    set_close_on_exec(&pair.master).context("protect the pseudo-terminal master")?;
    set_close_on_exec(&pair.slave).context("protect the pseudo-terminal slave")?;
    Ok((pair.master, pair.slave))
}

/// Attach the selected slave descriptor to the child streams.
///
/// # Examples
///
/// ```ignore
/// attach_streams(&mut command, Attach::StderrOnly, slave)?;
/// ```
fn attach_streams(command: &mut Command, attach: Attach, slave: OwnedFd) -> Result<()> {
    match attach {
        Attach::Both => {
            let stderr = slave
                .as_fd()
                .try_clone_to_owned()
                .context("clone terminal slave for stderr")?;
            command.stdout(Stdio::from(slave));
            command.stderr(Stdio::from(stderr));
        }
        Attach::StderrOnly => {
            command.stdout(Stdio::piped());
            command.stderr(Stdio::from(slave));
        }
        Attach::StdoutOnly => {
            command.stdout(Stdio::from(slave));
            command.stderr(Stdio::piped());
        }
    }
    Ok(())
}

/// Set close-on-exec while preserving the descriptor's existing flags.
///
/// # Examples
///
/// ```ignore
/// let pair = nix::pty::openpty(None, None)?;
/// set_close_on_exec(&pair.master)?;
/// ```
fn set_close_on_exec(descriptor: impl AsFd) -> Result<()> {
    let existing = fcntl(&descriptor, FcntlArg::F_GETFD).context("read descriptor flags")?;
    let flags = FdFlag::from_bits_truncate(existing) | FdFlag::FD_CLOEXEC;
    fcntl(descriptor, FcntlArg::F_SETFD(flags)).context("set close-on-exec flag")?;
    Ok(())
}

/// Take the unattached child output stream when the selected route pipes it.
///
/// # Examples
///
/// ```ignore
/// let stream = take_piped_stream(&mut child, Attach::StderrOnly)?;
/// assert!(stream.is_some());
/// ```
fn take_piped_stream(child: &mut Child, attach: Attach) -> Result<Option<Box<dyn Read + Send>>> {
    let description = match attach {
        Attach::Both => return Ok(None),
        Attach::StderrOnly => "standard output",
        Attach::StdoutOnly => "standard error",
    };
    let stream = match attach {
        Attach::StderrOnly => child
            .stdout
            .take()
            .map(|stream| Box::new(stream) as Box<dyn Read + Send>),
        Attach::StdoutOnly => child
            .stderr
            .take()
            .map(|stream| Box::new(stream) as Box<dyn Read + Send>),
        Attach::Both => return Ok(None),
    };
    stream
        .map(Some)
        .ok_or_else(|| anyhow!("{description} pipe was not created"))
}

/// Wait for the direct child and kill its process group if the deadline expires.
///
/// # Examples
///
/// ```ignore
/// let (status, timed_out) = wait_for_child(&mut child, group, timeout)?;
/// assert!(!timed_out || !status.success());
/// ```
pub(super) fn wait_for_child(
    child: &mut Child,
    process_group: Pid,
    timeout: Duration,
) -> Result<(ExitStatus, bool)> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().context("wait for pseudo-terminal child")? {
            return Ok((status, false));
        }
        if Instant::now() >= deadline {
            let status =
                stop_child(child, process_group).context("reap timed-out pseudo-terminal child")?;
            return Ok((status, true));
        }
        thread::sleep(POLL_INTERVAL);
    }
}

/// Kill a process group and reap its direct child.
///
/// # Examples
///
/// ```ignore
/// stop_child(&mut child, process_group)?;
/// ```
pub(super) fn stop_child(child: &mut Child, process_group: Pid) -> Result<ExitStatus> {
    kill_process_group(process_group)?;
    child.wait().context("reap pseudo-terminal child")
}

/// Kill a process group, accepting that its leader may already have exited.
///
/// # Examples
///
/// ```ignore
/// kill_process_group(process_group)?;
/// ```
pub(super) fn kill_process_group(process_group: Pid) -> Result<()> {
    match killpg(process_group, Signal::SIGKILL) {
        Ok(()) | Err(Errno::ESRCH) => Ok(()),
        Err(error) => Err(error).context("kill pseudo-terminal process group"),
    }
}
