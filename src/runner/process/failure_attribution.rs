//! Bounded extraction of command-list failure attribution from Ninja stderr.

use crate::ninja_gen::COMMAND_LIST_FAILURE_PREFIX;
use std::io::{self, Write};

use super::streaming::{ForwardStats, forward_child_output};

/// Forward stderr while retaining only the bounded command-list failure marker.
pub(super) fn forward_stderr_with_attribution<R, W>(
    reader: R,
    output: W,
) -> (ForwardStats, Option<String>)
where
    R: io::Read,
    W: Write,
{
    let mut attribution_writer = FailureAttributionWriter::new(output);
    let stats = forward_child_output(reader, &mut attribution_writer, "stderr");
    (stats, attribution_writer.into_failure())
}

pub(super) struct FailureAttributionWriter<W> {
    inner: W,
    pending: Vec<u8>,
    failure: Option<String>,
}

impl<W> FailureAttributionWriter<W> {
    const MAX_LINE_BYTES: usize = 128;

    pub(super) const fn new(inner: W) -> Self {
        Self {
            inner,
            pending: Vec::new(),
            failure: None,
        }
    }

    pub(super) fn into_failure(self) -> Option<String> {
        self.failure
    }

    fn observe(&mut self, bytes: &[u8]) {
        for byte in bytes {
            if *byte == b'\n' {
                self.record_line();
                self.pending.clear();
            } else if self.pending.len() < Self::MAX_LINE_BYTES {
                self.pending.push(*byte);
            }
        }
    }

    fn record_line(&mut self) {
        let Ok(line) = std::str::from_utf8(&self.pending) else {
            return;
        };
        let Some((action, entry)) = line
            .strip_prefix(COMMAND_LIST_FAILURE_PREFIX)
            .and_then(|suffix| suffix.split_once(", entry "))
            .and_then(|(action, entry)| {
                Some((action.parse::<usize>().ok()?, entry.parse::<usize>().ok()?))
            })
        else {
            return;
        };
        if action > 0 && entry > 0 {
            self.failure = Some(format!(
                "{COMMAND_LIST_FAILURE_PREFIX}{action}, entry {entry}"
            ));
        }
    }
}

impl<W: Write> Write for FailureAttributionWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = self.inner.write(bytes)?;
        let Some(written) = bytes.get(..count) else {
            return Err(io::Error::other("writer reported an invalid byte count"));
        };
        self.observe(written);
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    //! Tests for bounded, chunk-independent failure attribution.

    use super::*;

    #[test]
    fn extracts_a_valid_marker_split_across_writes() {
        let mut writer = FailureAttributionWriter::new(Vec::new());
        writer
            .write_all(b"ninja output\nnetsuke command-list fail")
            .expect("first chunk should write");
        writer
            .write_all(b"ure: action 7, entry 3\n")
            .expect("second chunk should write");

        assert_eq!(
            writer.into_failure().as_deref(),
            Some("netsuke command-list failure: action 7, entry 3")
        );
    }

    #[test]
    fn ignores_malformed_or_unbounded_markers() {
        let mut writer = FailureAttributionWriter::new(Vec::new());
        writer
            .write_all(b"netsuke command-list failure: action zero, entry 2\n")
            .expect("malformed marker should write");
        writer
            .write_all(&[b'x'; FailureAttributionWriter::<Vec<u8>>::MAX_LINE_BYTES + 1])
            .expect("unbounded marker should write");
        writer
            .write_all(b"netsuke command-list failure: action 7, entry 3\n")
            .expect("valid marker after unbounded content should write");

        assert!(writer.into_failure().is_none());
    }
}
