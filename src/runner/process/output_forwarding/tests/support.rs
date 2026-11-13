//! Fixtures shared by the forwarding-boundary tests.
//!
//! Each fixture fails at exactly one point — reading, writing, or draining — so
//! a case names the failure it drives instead of inferring it from output.

use std::io::{self, Read, Write};

/// Writer that rejects every chunk, as a closed destination pipe does.
pub(super) struct RejectingWriter;

impl Write for RejectingWriter {
    fn write(&mut self, _bytes: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "destination closed",
        ))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Reader that reports one failure and then reads as exhausted.
///
/// Failing only once keeps the failure attributable to the read the copy loop
/// performs, rather than to the drain that follows it.
pub(super) struct FailsOnce {
    /// Whether the failure has already been reported.
    reported: bool,
}

impl FailsOnce {
    /// Report one failure, then behave as an exhausted reader.
    pub(super) const fn new() -> Self {
        Self { reported: false }
    }
}

impl Default for FailsOnce {
    fn default() -> Self {
        Self::new()
    }
}

impl Read for FailsOnce {
    fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
        if self.reported {
            return Ok(0);
        }
        self.reported = true;
        Err(io::Error::other("read failed"))
    }
}

/// Reader that yields one chunk and then fails while the copy loop drains.
pub(super) struct FailsWhileDraining {
    /// Bytes delivered by the first read.
    bytes: Vec<u8>,
    /// Whether the chunk has already been delivered.
    delivered: bool,
}

impl FailsWhileDraining {
    /// Deliver `bytes` once, then fail every later read.
    pub(super) const fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            delivered: false,
        }
    }
}

impl Read for FailsWhileDraining {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.delivered {
            return Err(io::Error::other("drain read failed"));
        }
        self.delivered = true;
        let count = self.bytes.len().min(buffer.len());
        let (Some(source), Some(destination)) = (self.bytes.get(..count), buffer.get_mut(..count))
        else {
            // `count` is the smaller of the two lengths, so both slices always
            // exist; report EOF rather than panicking if that changes.
            return Ok(0);
        };
        destination.copy_from_slice(source);
        Ok(count)
    }
}
