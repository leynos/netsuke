//! Prove that the PTY harness assigns terminal state and drains both streams.

use super::{Attach, PtyRequest, readers::is_pty_eof, run_in_pty};
use nix::errno::Errno;
use rstest::rstest;
use std::{io, process::Command, time::Duration};

#[rstest]
#[case::eio(Errno::EIO as i32, true)]
#[case::bad_file_descriptor(Errno::EBADF as i32, false)]
fn pty_eof_recognizes_only_eio(#[case] raw_error: i32, #[case] expected: bool) {
    let error = io::Error::from_raw_os_error(raw_error);

    assert_eq!(is_pty_eof(&error), expected);
}

#[test]
fn pty_smoke_attaches_both_output_streams() {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "test -t 1 && test -t 2 && printf ok"]);
    let output = run_in_pty(PtyRequest {
        command,
        attach: Attach::Both,
        timeout: Duration::from_secs(5),
    })
    .expect("PTY child should run with both streams attached");

    assert!(output.status.success());
    assert!(output.terminal.ends_with(b"ok"));
    assert_eq!(output.piped.as_slice(), b"");
}

#[test]
fn pty_smoke_attaches_stderr_and_pipes_stdout() {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "test ! -t 1 && test -t 2 && printf terminal >&2"]);
    let output = run_in_pty(PtyRequest {
        command,
        attach: Attach::StderrOnly,
        timeout: Duration::from_secs(5),
    })
    .expect("PTY child should run with stderr attached");

    assert!(output.status.success());
    assert!(output.terminal.ends_with(b"terminal"));
    assert_eq!(output.piped.as_slice(), b"");
}

#[test]
fn pty_smoke_attaches_stdout_and_pipes_stderr() {
    let mut command = Command::new("/bin/sh");
    command.args([
        "-c",
        "test -t 1 && test ! -t 2 && printf terminal && printf piped >&2",
    ]);
    let output = run_in_pty(PtyRequest {
        command,
        attach: Attach::StdoutOnly,
        timeout: Duration::from_secs(5),
    })
    .expect("PTY child should run with stdout attached");

    assert!(output.status.success());
    assert!(output.terminal.ends_with(b"terminal"));
    assert_eq!(output.piped, b"piped");
}

/// A child that outstays its timeout must be reported and killed.
///
/// Without this the harness could return a truncated capture from a hung child
/// as if it were a completed run, and every assertion built on that capture
/// would quietly measure less than it claims.
#[test]
fn pty_reports_a_child_that_exceeds_its_timeout() {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "sleep 30"]);
    let started = std::time::Instant::now();
    let error = run_in_pty(PtyRequest {
        command,
        attach: Attach::Both,
        timeout: Duration::from_millis(200),
    })
    .expect_err("a child that outlives its timeout must fail the run");

    let elapsed = started.elapsed();
    assert!(
        format!("{error:#}").contains("exceeded"),
        "the timeout must be named in the error: {error:#}"
    );
    assert!(
        elapsed < Duration::from_secs(10),
        "the run must stop near its timeout, not wait for the child: {elapsed:?}"
    );
}

/// A descendant holding the terminal open must not keep the capture blocked.
///
/// The direct child exits immediately, but its background child still owns the
/// pseudo-terminal. A harness that waits only for the direct child would block
/// until that descendant finished, so this pins the process-group kill rather
/// than the ordinary exit path.
#[test]
fn pty_stops_reading_when_a_descendant_holds_the_terminal() {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "sleep 30 & printf started; exit 0"]);
    let started = std::time::Instant::now();
    let output = run_in_pty(PtyRequest {
        command,
        attach: Attach::Both,
        timeout: Duration::from_secs(5),
    })
    .expect("the descendant must be stopped rather than waited for");

    let elapsed = started.elapsed();
    assert_eq!(
        output.terminal.as_slice(),
        b"started",
        "the direct child's output must still be captured"
    );
    assert!(
        elapsed < Duration::from_secs(20),
        "the descendant must not hold the capture open: {elapsed:?}"
    );
}
