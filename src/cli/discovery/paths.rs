//! Path comparison helpers for configuration discovery.

use std::io;
use std::path::{Path, PathBuf};

/// Return the expected project file, or `None` without a working directory.
pub(super) fn project_scope_file(directory: Option<&Path>) -> Option<PathBuf> {
    let root = directory
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())?;
    Some(root.join(".netsuke.toml"))
}

/// Compare a canonical project path, falling back literally if resolution fails.
///
/// Missing optional files must not fail discovery. The project loading pass
/// can still compare canonical chain roots if normalization is unavailable.
pub(super) fn comparison_key(normalizer: &impl PathNormalizer, path: &str) -> PathBuf {
    normalized_path_key(normalizer, path).unwrap_or_else(|_| PathBuf::from(path))
}

/// Resolves a path to the canonical form used for layer comparison.
///
/// This is the seam through which discovery reaches the filesystem, so tests can
/// force a resolution failure without depending on the ambient environment.
pub(super) trait PathNormalizer {
    /// Resolve `path`, propagating any I/O error to the caller.
    fn normalize(&self, path: &Path) -> io::Result<PathBuf>;

    /// Report whether `path` names an existing filesystem entry.
    ///
    /// Callers use this only to decide what an *unresolvable* path means, so the
    /// query stays on this seam rather than reaching for `Path::exists`. That
    /// inherent method collapses every I/O error into `false`, which would let
    /// an unreadable directory entry masquerade as an absent file; reporting the
    /// error instead keeps that distinction available to the caller.
    fn exists(&self, path: &Path) -> io::Result<bool>;
}

/// Production normalizer that exactly mirrors `OrthoConfig`'s canonical paths.
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct FsPathNormalizer;

impl PathNormalizer for FsPathNormalizer {
    fn normalize(&self, path: &Path) -> io::Result<PathBuf> {
        // `ortho_config` canonicalizes layer paths with `dunce` on Windows so
        // diagnostics and comparisons stay free of UNC prefixes and short-name
        // forms; mirror that here so the project-scope dedup key and the
        // injected explicit config path compare equal to the recorded layer
        // path. On other platforms `dunce` is a thin wrapper over
        // `std::fs::canonicalize`, so the behaviour is unchanged.
        dunce::canonicalize(path)
    }

    fn exists(&self, path: &Path) -> io::Result<bool> {
        path.try_exists()
    }
}

/// Return the canonical comparison key for `path`.
///
/// `OrthoConfig` canonicalizes every layer path it records, whereas the expected
/// project path is joined from the caller's `--directory` verbatim. Normalizing
/// the expected project path keeps a relative or symlinked directory from
/// looking like a different file without re-reading each stored layer path.
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

    /// Defer existence to the real filesystem, which the fixture controls.
    ///
    /// This double models a path that will not *resolve*; the cases that use it
    /// distinguish a missing project file from an existing one, so the
    /// existence answer must still be genuine.
    fn exists(&self, path: &Path) -> io::Result<bool> {
        FsPathNormalizer.exists(path)
    }
}

/// A normalizer that cannot resolve a path or report whether it exists.
///
/// An unreadable directory entry is not the same fact as a missing file, and
/// discovery must be able to tell the two apart. The real filesystem produces
/// this state for the agent's own tests only if permissions are arranged just
/// so, which a test cannot do portably.
#[cfg(test)]
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct UnreadablePathNormalizer;

#[cfg(test)]
impl PathNormalizer for UnreadablePathNormalizer {
    fn normalize(&self, _path: &Path) -> io::Result<PathBuf> {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "normalization refused for test",
        ))
    }

    fn exists(&self, _path: &Path) -> io::Result<bool> {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "existence query refused for test",
        ))
    }
}
