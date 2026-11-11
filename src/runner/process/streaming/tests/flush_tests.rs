//! Verify each forwarded output chunk becomes visible before the next read.

use super::super::ForwardRoute;
use super::{ChunkedReader, ForwardFailureKind, ForwardStream, forward_child_output, record_each};
use proptest::prelude::*;
use std::{
    cell::RefCell,
    io::{self, Read, Write},
    rc::Rc,
};

#[derive(Debug, PartialEq, Eq)]
enum ForwardingEvent {
    Read(usize),
    Flush,
}

struct ObservedReader<R> {
    inner: R,
    events: Rc<RefCell<Vec<ForwardingEvent>>>,
}

impl<R: Read> Read for ObservedReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let count = self.inner.read(buffer)?;
        self.events.borrow_mut().push(ForwardingEvent::Read(count));
        Ok(count)
    }
}

#[derive(Default)]
struct FlushBufferWriter {
    pending: Vec<u8>,
    visible: Vec<u8>,
    snapshots: Vec<Vec<u8>>,
    maximum_write: usize,
    write_count: usize,
    events: Rc<RefCell<Vec<ForwardingEvent>>>,
}

impl FlushBufferWriter {
    fn new(maximum_write: usize, events: Rc<RefCell<Vec<ForwardingEvent>>>) -> Self {
        Self {
            maximum_write,
            events,
            ..Self::default()
        }
    }
}

impl Write for FlushBufferWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.write_count = self.write_count.saturating_add(1);
        let count = bytes.len().min(self.maximum_write);
        let Some(prefix) = bytes.get(..count) else {
            return Err(io::Error::other("writer prefix is outside the input"));
        };
        self.pending.extend_from_slice(prefix);
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.events.borrow_mut().push(ForwardingEvent::Flush);
        self.visible.append(&mut self.pending);
        self.snapshots.push(self.visible.clone());
        Ok(())
    }
}

proptest! {
    #[test]
    fn copy_and_flush_preserves_generated_chunks_and_flush_order(
        chunks in prop::collection::vec(
            prop::collection::vec(any::<u8>(), 1..20),
            0..9,
        ),
        maximum_write in 1_usize..9,
    ) {
        let input = chunks.iter().flatten().copied().collect::<Vec<_>>();
        let chunk_sizes = chunks.iter().map(Vec::len).collect::<Vec<_>>();
        let events = Rc::default();
        let mut reader = ObservedReader {
            inner: ChunkedReader::new(input.clone(), chunk_sizes),
            events: Rc::clone(&events),
        };
        let mut writer = FlushBufferWriter::new(maximum_write, Rc::clone(&events));

        let result = super::super::copy_and_flush(&mut reader, &mut writer);
        prop_assert!(result.is_ok(), "copy_and_flush failed: {:?}", result.err());
        prop_assert_eq!(&writer.visible, &input);

        let mut visible = Vec::new();
        let expected_snapshots = chunks.iter().map(|chunk| {
            visible.extend_from_slice(chunk);
            visible.clone()
        }).collect::<Vec<_>>();
        prop_assert_eq!(&writer.snapshots, &expected_snapshots);

        let expected_write_count = chunks.iter()
            .map(|chunk| chunk.len().div_ceil(maximum_write))
            .sum::<usize>();
        prop_assert_eq!(writer.write_count, expected_write_count);

        let mut expected_events = Vec::new();
        for chunk in &chunks {
            expected_events.push(ForwardingEvent::Read(chunk.len()));
            expected_events.push(ForwardingEvent::Flush);
        }
        expected_events.push(ForwardingEvent::Read(0));
        prop_assert_eq!(&*events.borrow(), &expected_events);
    }
}

#[test]
fn forward_output_flushes_each_complete_chunk_before_reading_more() {
    let input = b"firstlast".to_vec();
    let events = Rc::default();
    let reader = ObservedReader {
        inner: ChunkedReader::new(input.clone(), vec![5, 4]),
        events: Rc::clone(&events),
    };
    let mut writer = FlushBufferWriter::new(2, Rc::clone(&events));

    let stats = forward_child_output(
        reader,
        &mut writer,
        ForwardRoute::new(ForwardStream::Stdout, &mut record_each),
    );

    assert_eq!(stats.bytes_read, input.len());
    assert_eq!(stats.bytes_written, input.len());
    assert_eq!(stats.failure, None);
    assert_eq!(
        *events.borrow(),
        vec![
            ForwardingEvent::Read(5),
            ForwardingEvent::Flush,
            ForwardingEvent::Read(4),
            ForwardingEvent::Flush,
            ForwardingEvent::Read(0),
        ]
    );
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

    let stats = forward_child_output(
        reader,
        &mut writer,
        ForwardRoute::new(ForwardStream::Stdout, &mut record_each),
    );

    assert_eq!(writer.accepted, b"first");
    assert_eq!(writer.write_count, 1, "accepted bytes must not be retried");
    assert_eq!(writer.flush_count, 1);
    assert_eq!(stats.bytes_read, input.len(), "remaining bytes are drained");
    assert_eq!(stats.bytes_written, b"first".len());
    assert_eq!(stats.failure, Some(ForwardFailureKind::Flush));
}
