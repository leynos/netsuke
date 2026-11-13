//! Verify status parsing keeps its memory footprint bounded.
//!
//! The ignored case forwards a large stream with and without status parsing
//! and compares peak resident-set size, so a parser that retained its input
//! would fail rather than pass on byte counts alone.

use super::super::{
    ForwardRoute, ForwardStream, forward_child_output, forward_child_output_with_ninja_status,
};
use super::record_each;
use std::io::{self, Read};

/// Generate a large stream without retaining its forwarded bytes in memory.
#[cfg(unix)]
struct RepeatingByteReader {
    /// Bytes still available to read.
    remaining: usize,
}

#[cfg(unix)]
impl RepeatingByteReader {
    /// Construct a reader producing `byte_count` bytes without newlines.
    const fn new(byte_count: usize) -> Self {
        Self {
            remaining: byte_count,
        }
    }
}

#[cfg(unix)]
impl Read for RepeatingByteReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let count = self.remaining.min(buf.len());
        let Some(destination) = buf.get_mut(..count) else {
            return Err(io::Error::other(
                "repeating reader destination range is invalid",
            ));
        };
        destination.fill(b'x');
        self.remaining = self.remaining.saturating_sub(count);
        Ok(count)
    }
}

/// Return the process's current peak resident-set size in bytes.
#[cfg(unix)]
fn peak_rss_bytes() -> io::Result<u64> {
    let usage = nix::sys::resource::getrusage(nix::sys::resource::UsageWho::RUSAGE_SELF)
        .map_err(io::Error::other)?;
    let resident_set = u64::try_from(usage.max_rss())
        .map_err(|_| io::Error::other("getrusage returned a negative RSS"))?;
    #[cfg(target_os = "macos")]
    let bytes = resident_set;
    #[cfg(not(target_os = "macos"))]
    let bytes = resident_set.saturating_mul(1024);
    Ok(bytes)
}

#[cfg(unix)]
#[test]
#[ignore = "getrusage is process-global and varies with allocator and OS accounting"]
fn status_parsing_peak_rss_does_not_scale_with_a_large_unterminated_stream() {
    const STREAM_BYTES: usize = 256 * 1024 * 1024;
    const RSS_ALLOWANCE_BYTES: u64 = 4 * 1024 * 1024;

    let plain_before = peak_rss_bytes().expect("getrusage should report initial RSS");
    let plain_stats = forward_child_output(
        RepeatingByteReader::new(STREAM_BYTES),
        io::sink(),
        ForwardRoute::new(ForwardStream::Stdout, &mut record_each),
    );
    let plain_after = peak_rss_bytes().expect("getrusage should report plain forwarding RSS");
    let status_before = peak_rss_bytes().expect("getrusage should report status forwarding RSS");
    let status_stats = forward_child_output_with_ninja_status(
        RepeatingByteReader::new(STREAM_BYTES),
        io::sink(),
        |_current, _total, _description| {},
        ForwardRoute::new(ForwardStream::Stdout, &mut record_each),
    );
    let status_after = peak_rss_bytes().expect("getrusage should report status parsing RSS");

    assert_eq!(plain_stats.bytes_read, STREAM_BYTES);
    assert_eq!(status_stats.bytes_read, STREAM_BYTES);
    assert!(plain_after >= plain_before);
    assert!(status_after >= status_before);
    assert!(
        status_after.saturating_sub(status_before) <= RSS_ALLOWANCE_BYTES,
        "status parsing should add only a fixed RSS allowance, not one proportional to {STREAM_BYTES} bytes"
    );
}
