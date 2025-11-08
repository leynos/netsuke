//! Result types shared across command helpers.

use camino::Utf8PathBuf;

#[derive(Debug)]
pub(super) enum StdoutResult {
    Bytes(Vec<u8>),
    Tempfile(Utf8PathBuf),
}

#[derive(Debug)]
pub(super) enum PipeOutcome {
    Bytes(Vec<u8>),
    Tempfile(Utf8PathBuf),
}
