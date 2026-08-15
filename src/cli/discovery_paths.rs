//! Path comparison helpers for configuration discovery.

use std::io;
use std::path::{Path, PathBuf};

/// Resolves a path to the canonical form used for layer comparison.
///
/// This is the seam through which discovery reaches the filesystem, so tests can
/// force a resolution failure without depending on the ambient environment.
pub(super) trait PathNormalizer {
    /// Resolve `path`, propagating any I/O error to the caller.
    fn normalize(&self, path: &Path) -> io::Result<PathBuf>;
}

/// Production normalizer that exactly mirrors `OrthoConfig`'s canonical paths.
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct FsPathNormalizer;

impl PathNormalizer for FsPathNormalizer {
    fn normalize(&self, path: &Path) -> io::Result<PathBuf> {
        // `ortho_config` canonicalises layer paths with `dunce` on Windows so
        // diagnostics and comparisons stay free of UNC prefixes; mirror that
        // here so the project-scope dedup key matches the recorded layer path.
        #[cfg(windows)]
        {
            dunce::canonicalize(path)
        }
        #[cfg(not(windows))]
        {
            std::fs::canonicalize(path)
        }
    }
}

/// Return the canonical comparison key for `path`.
///
/// `OrthoConfig` canonicalizes every layer path it records, whereas the expected
/// project path is joined from the caller's `--directory` verbatim. Passing both
/// sides through the same normalizer keeps a relative or symlinked directory
/// from looking like a different file.
///
/// Resolution failure is reported rather than absorbed: the caller decides what
/// an unresolvable path means. See `collect_file_layers`, which compares such a
/// path literally so a missing project `.netsuke.toml` or an unreadable
/// directory does not fail configuration discovery.
pub(super) fn normalized_path_key(
    normalizer: &impl PathNormalizer,
    path: &str,
) -> io::Result<PathBuf> {
    normalizer.normalize(Path::new(path))
}

/// A normalizer that always fails, so the failure branch is deterministic.
///
/// Real canonicalization failure depends on the ambient filesystem — a missing
/// file, an unreadable directory — which a test cannot force portably.
#[cfg(test)]
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct FailingPathNormalizer;

#[cfg(test)]
impl PathNormalizer for FailingPathNormalizer {
    fn normalize(&self, _path: &Path) -> io::Result<PathBuf> {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "normalization refused for test",
        ))
    }
}
