//! Request capture for the local HTTP fixture.
//!
//! The fixture answers the requests it receives and exposes their request
//! lines, so a test can assert the method and path a client used without
//! running a real server. Capture is bounded: a client that never completes a
//! header block is released when the configured read deadline passes.

use std::{
    io::{self, Read},
    net::TcpStream,
    sync::{Arc, Mutex, MutexGuard},
    thread,
    time::{Duration, Instant},
};

/// Maximum number of request bytes captured before the fixture responds.
const MAX_REQUEST_BYTES: usize = 8 * 1024;

/// Request lines recorded by one fixture server, in arrival order.
#[derive(Clone, Debug, Default)]
pub struct RequestLog {
    /// Recorded request lines behind a shared handle.
    lines: Arc<Mutex<Vec<String>>>,
}

impl RequestLog {
    /// Return the recorded request lines, in arrival order.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        self.lock().clone()
    }

    /// Return the number of requests recorded so far.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    /// Report whether the fixture has recorded no request yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lock().is_empty()
    }

    /// Record one client request line.
    pub(super) fn record(&self, line: String) {
        self.lock().push(line);
    }

    /// Lock the log, recovering the guard from a poisoned mutex.
    fn lock(&self) -> MutexGuard<'_, Vec<String>> {
        match self.lines.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

/// Readiness reported by one non-blocking read from a fixture client.
enum ReadStep {
    /// The client closed its side of the connection.
    Eof,
    /// More request bytes were captured.
    Captured,
    /// No byte is ready yet.
    Blocked,
}

/// Read one request, returning its request line when the client sent one.
///
/// Accumulates bytes until the header block ends, the capture bound is
/// reached, the client disconnects, or `deadline` passes. Returns `None` when
/// the client sent nothing, which is how the fixture distinguishes an
/// abandoned chain from a request it must answer.
pub(super) fn read_request_line(
    stream: &mut TcpStream,
    deadline: Instant,
    poll_interval: Duration,
) -> Option<String> {
    let mut captured = Vec::new();
    loop {
        match try_read(stream, &mut captured) {
            ReadStep::Eof => break,
            ReadStep::Captured => {
                if captured.len() >= MAX_REQUEST_BYTES || header_block_complete(&captured) {
                    break;
                }
            }
            ReadStep::Blocked => {
                if Instant::now() >= deadline {
                    break;
                }
                thread::sleep(poll_interval);
            }
        }
    }
    if captured.is_empty() {
        return None;
    }
    Some(request_line(&captured))
}

/// Append one chunk of request bytes to `captured`, reporting readiness.
#[expect(clippy::panic, reason = "tests panic to surface unexpected IO errors")]
fn try_read(stream: &mut TcpStream, captured: &mut Vec<u8>) -> ReadStep {
    let mut buf = [0_u8; 1024];
    match stream.read(&mut buf) {
        Ok(0) => ReadStep::Eof,
        Ok(read) => {
            captured.extend_from_slice(buf.get(..read).unwrap_or(&buf));
            ReadStep::Captured
        }
        Err(err) if err.kind() == io::ErrorKind::WouldBlock => ReadStep::Blocked,
        Err(err) => panic!("failed to read request: {err}"),
    }
}

/// Report whether `bytes` holds a complete request header block.
fn header_block_complete(bytes: &[u8]) -> bool {
    bytes.windows(4).any(|window| window == b"\r\n\r\n")
}

/// Extract the request line from captured request bytes.
fn request_line(bytes: &[u8]) -> String {
    let end = bytes
        .windows(2)
        .position(|window| window == b"\r\n")
        .or_else(|| bytes.iter().position(|byte| *byte == b'\n'))
        .unwrap_or(bytes.len());
    let line = bytes.get(..end).unwrap_or(bytes);
    String::from_utf8_lossy(line).into_owned()
}
