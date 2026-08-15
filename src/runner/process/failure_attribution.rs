//! Bounded extraction of command-list failure attribution from Ninja output.

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

/// Retain only Ninja's trailing output, where it relays failed subcommand
/// diagnostics after the command itself has completed.
///
/// Ninja merges a subcommand's stderr into its own stdout. Retaining a small
/// tail lets the process boundary recover the generated failure marker after a
/// non-zero exit without examining or line-buffering ordinary command output.
pub(super) struct NinjaFailureOutputTail<W> {
    inner: W,
    tail: Vec<u8>,
}

impl<W> NinjaFailureOutputTail<W> {
    const MAX_TAIL_BYTES: usize = 512;

    pub(super) fn new(inner: W) -> Self {
        Self {
            inner,
            tail: Vec::with_capacity(Self::MAX_TAIL_BYTES),
        }
    }

    /// Extract a bounded marker only after Ninja has reported a failure.
    pub(super) fn into_failure(self) -> Option<CommandListFailure> {
        self.tail
            .split(|byte| *byte == b'\n')
            .filter_map(parse_marker)
            .next_back()
    }

    #[cfg(test)]
    const fn tail_len(&self) -> usize {
        self.tail.len()
    }

    fn retain_tail(&mut self, bytes: &[u8]) {
        if bytes.len() >= Self::MAX_TAIL_BYTES {
            self.tail.clear();
            let suffix = bytes
                .get(bytes.len().saturating_sub(Self::MAX_TAIL_BYTES)..)
                .unwrap_or_default();
            self.tail.extend_from_slice(suffix);
            return;
        }

        let retained = self.tail.len().saturating_add(bytes.len());
        if retained > Self::MAX_TAIL_BYTES {
            self.tail.drain(..retained - Self::MAX_TAIL_BYTES);
        }
        self.tail.extend_from_slice(bytes);
    }
}

impl<W: Write> Write for NinjaFailureOutputTail<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = self.inner.write(bytes)?;
        let Some(written) = bytes.get(..count) else {
            return Err(io::Error::other("writer reported an invalid byte count"));
        };
        self.retain_tail(written);
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
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
        self.failure = parse_marker(&self.pending);
    }
}

fn parse_marker(bytes: &[u8]) -> Option<CommandListFailure> {
    let line = std::str::from_utf8(bytes).ok()?;
    let (action, entry_text) = line
        .strip_prefix(COMMAND_LIST_FAILURE_PREFIX)?
        .split_once(", entry ")?;
    let entry = entry_text.parse::<usize>().ok()?;
    if is_action_identity(action) && entry > 0 {
        Some(CommandListFailure {
            action_identity: action.to_owned(),
            entry_index: entry,
        })
    } else {
        None
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

    #[test]
    fn retains_a_bounded_ninja_output_tail_for_failure_attribution() {
        let mut writer = NinjaFailureOutputTail::new(Vec::new());
        writer
            .write_all(&vec![b'x'; 256 * 1024])
            .expect("large command output should forward");
        writer
            .write_all(
                format!("\nnetsuke command-list failure: action {ACTION_IDENTITY}, entry 3\n")
                    .as_bytes(),
            )
            .expect("Ninja failure marker should forward");

        assert!(
            writer.tail_len() <= NinjaFailureOutputTail::<Vec<u8>>::MAX_TAIL_BYTES,
            "Ninja failure attribution must retain a fixed-size output tail"
        );
        assert_eq!(
            writer.into_failure().map(|failure| failure.to_string()),
            Some(
                "netsuke command-list failure: action 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef, entry 3"
                    .into()
            )
        );
    }
}
