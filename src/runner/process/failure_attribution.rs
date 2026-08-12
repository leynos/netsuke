//! Bounded extraction of command-list failure attribution from Ninja stderr.

use crate::ninja_gen::ninja_gen_command_list::COMMAND_LIST_FAILURE_PREFIX;
use std::io::{self, Write};

use super::streaming::{ForwardStats, forward_child_output};

/// Forward stderr while retaining only the bounded command-list failure marker.
pub(super) fn forward_stderr_with_attribution<R, W>(
    reader: R,
    output: W,
) -> (ForwardStats, Option<CommandListFailure>)
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
    failure: Option<CommandListFailure>,
}

/// Safe, fixed-shape failure details emitted by command-list lowering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CommandListFailure {
    action_identity: String,
    entry_index: usize,
}

impl CommandListFailure {
    /// Stable hashed action identity, never the manifest command content.
    pub(super) fn action_identity(&self) -> &str {
        &self.action_identity
    }

    /// One-based command-list entry position.
    pub(super) const fn entry_index(&self) -> usize {
        self.entry_index
    }
}

impl std::fmt::Display for CommandListFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{COMMAND_LIST_FAILURE_PREFIX}{}, entry {}",
            self.action_identity, self.entry_index
        )
    }
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

    pub(super) fn into_failure(self) -> Option<CommandListFailure> {
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
            .and_then(|(action, entry)| Some((action, entry.parse::<usize>().ok()?)))
        else {
            return;
        };
        if is_action_identity(action) && entry > 0 {
            self.failure = Some(CommandListFailure {
                action_identity: action.to_owned(),
                entry_index: entry,
            });
        }
    }
}

fn is_action_identity(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
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

    const ACTION_IDENTITY: &str =
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn extracts_a_valid_marker_split_across_writes() {
        let mut writer = FailureAttributionWriter::new(Vec::new());
        writer
            .write_all(b"ninja output\nnetsuke command-list fail")
            .expect("first chunk should write");
        writer
            .write_all(format!("ure: action {ACTION_IDENTITY}, entry 3\n").as_bytes())
            .expect("second chunk should write");

        let failure = writer.into_failure().map(|failure| failure.to_string());
        assert_eq!(
            failure.as_deref(),
            Some(
                "netsuke command-list failure: action 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef, entry 3"
            )
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
            .write_all(
                format!("netsuke command-list failure: action {ACTION_IDENTITY}, entry 3\n")
                    .as_bytes(),
            )
            .expect("valid marker after unbounded content should write");

        assert!(writer.into_failure().is_none());
    }
}
