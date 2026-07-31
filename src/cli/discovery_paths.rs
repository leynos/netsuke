//! Path comparison helpers for configuration discovery.

use std::path::{Path, PathBuf};

/// Return a comparable key for `path`, resolving it where the file exists.
///
/// `OrthoConfig` canonicalises every layer path it records, whereas the expected
/// project path is joined from the caller's `--directory` verbatim. Passing both
/// sides through this function keeps a relative or symlinked directory from
/// looking like a different file.
pub(super) fn normalized_path_key(path: &str) -> PathBuf {
    let candidate = Path::new(path);
    std::fs::canonicalize(candidate).unwrap_or_else(|_| candidate.to_path_buf())
}
