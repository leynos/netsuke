//! Path comparison helpers for configuration discovery.

use std::path::{Path, PathBuf};

/// Return a comparable key for `path`, resolving it where the file exists.
///
/// `OrthoConfig` canonicalizes every layer path it records, whereas the expected
/// project path is joined from the caller's `--directory` verbatim. Passing both
/// sides through this function keeps a relative or symlinked directory from
/// looking like a different file.
///
/// # Filesystem access and fallback policy
///
/// This reads the filesystem through [`std::fs::canonicalize`], so it is not a
/// pure function. Resolution failure is deliberately **not** an error: the
/// common input is the expected `.netsuke.toml`, which usually does not exist,
/// and a directory may equally be unreadable. The function is therefore total —
/// an unresolvable path is returned unchanged and compared literally.
///
/// Returning `Result` would misreport the ordinary "no project config" case as a
/// failure, and every caller would have to reapply this same fallback to get a
/// key it can compare. Comparing literally is also sound: an unresolved path
/// cannot equal a resolved one, so the caller simply treats the layer as
/// unmatched and takes the append branch, which is the safe direction.
///
/// The policy is pinned by `normalized_path_key_is_identity_for_absent_paths`
/// and `normalized_path_key_is_idempotent`, with `.`/`..` resolution covered by
/// `normalized_path_key_resolves_non_canonical_forms`.
pub(super) fn normalized_path_key(path: &str) -> PathBuf {
    let candidate = Path::new(path);
    std::fs::canonicalize(candidate).unwrap_or_else(|_| candidate.to_path_buf())
}
