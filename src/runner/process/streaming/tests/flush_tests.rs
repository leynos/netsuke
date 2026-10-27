//! Verify each forwarded output chunk becomes visible before the next read.

use super::{ChunkedReader, forward_child_output};
use std::io::{self, Write};

#[derive(Default)]
struct FlushBufferWriter {
    pending: Vec<u8>,
    visible: Vec<u8>,
    snapshots: Vec<Vec<u8>>,
    maximum_write: usize,
}

impl FlushBufferWriter {
    fn new(maximum_write: usize) -> Self {
        Self {
            maximum_write,
            ..Self::default()
        }
    }
}

impl Write for FlushBufferWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = bytes.len().min(self.maximum_write);
        let Some(prefix) = bytes.get(..count) else {
            return Err(io::Error::other("writer prefix is outside the input"));
        };
        self.pending.extend_from_slice(prefix);
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.visible.append(&mut self.pending);
        self.snapshots.push(self.visible.clone());
        Ok(())
    }
}

#[test]
fn forward_output_flushes_each_complete_chunk_before_reading_more() {
    let input = b"firstlast".to_vec();
    let reader = ChunkedReader::new(input.clone(), vec![5, 4]);
    let mut writer = FlushBufferWriter::new(2);

    let stats = forward_child_output(reader, &mut writer, "stdout");

    assert_eq!(stats.bytes_read, input.len());
    assert_eq!(stats.bytes_written, input.len());
    assert!(!stats.write_failed);
    assert_eq!(
        writer.snapshots,
        vec![b"first".to_vec(), b"firstlast".to_vec()]
    );
}

struct FailingFlushWriter {
    accepted: Vec<u8>,
    write_count: usize,
    flush_count: usize,
}

impl Write for FailingFlushWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.write_count += 1;
        self.accepted.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flush_count += 1;
        Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "destination flush failed",
        ))
    }
}

#[test]
fn flush_failure_keeps_accepted_bytes_and_drains_the_remaining_input() {
    let input = b"firstlast".to_vec();
    let reader = ChunkedReader::new(input.clone(), vec![5, 4]);
    let mut writer = FailingFlushWriter {
        accepted: Vec::new(),
        write_count: 0,
        flush_count: 0,
    };

    let stats = forward_child_output(reader, &mut writer, "stdout");

    assert_eq!(writer.accepted, b"first");
    assert_eq!(writer.write_count, 1, "accepted bytes must not be retried");
    assert_eq!(writer.flush_count, 1);
    assert_eq!(stats.bytes_read, input.len(), "remaining bytes are drained");
    assert_eq!(stats.bytes_written, b"first".len());
    assert!(stats.write_failed);
}
