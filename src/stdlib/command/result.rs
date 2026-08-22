//! Result types shared across command helpers.

use camino::Utf8PathBuf;

/// The materialized standard output of a completed command helper.
#[derive(Debug)]
pub(super) enum StdoutResult {
    /// Output captured in memory as raw bytes.
    Bytes(Vec<u8>),
    /// Output streamed to a tempfile, identified by its absolute path.
    Tempfile(Utf8PathBuf),
}

/// The outcome of draining a single pipe reader.
#[derive(Debug)]
pub(super) enum PipeOutcome {
    /// Output buffered in memory as raw bytes.
    Bytes(Vec<u8>),
    /// Output written to a tempfile, identified by its absolute path.
    Tempfile(Utf8PathBuf),
}
