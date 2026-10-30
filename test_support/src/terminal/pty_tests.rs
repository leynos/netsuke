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
