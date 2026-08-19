//! Path canonicalization for fixtures staged in ambient temporary
//! directories.
//!
//! Split from `fs.rs` to keep that module within the Whitaker
//! `module_max_lines` cap; included from there via `#[path]` so the helper
//! stays a child module of `fs`.

use camino::{Utf8Path, Utf8PathBuf};

/// Resolve `path` to the filesystem's canonical spelling.
///
/// The helper returns a [`camino::Utf8PathBuf`] so fixture code never has to
/// convert an ambient `std::path::PathBuf` back into the `camino` world. The
/// underlying canonicalization still happens through the ambient boundary that
/// `fs` exists to provide: `cap_std`'s `Dir::canonicalize` resolves only
/// within a directory handle and returns a relative path, so it cannot
/// reproduce an absolute canonical path for a fixture staged in an ambient
/// temporary directory.
///
/// # Errors
///
/// Propagates the underlying `std::fs::canonicalize` failure, or
/// [`std::io::ErrorKind::InvalidData`] when the canonical path is not valid
/// UTF-8.
///
/// # Examples
///
/// ```
/// use camino::Utf8Path;
///
/// let dir = tempfile::tempdir().expect("create tempdir");
/// let path = Utf8Path::from_path(dir.path()).expect("tempdir path is UTF-8");
/// let canonical = test_support::fs::canonicalize(path).expect("canonicalize fixture");
/// assert!(canonical.is_absolute());
/// ```
pub fn canonicalize(path: &Utf8Path) -> std::io::Result<Utf8PathBuf> {
    let canonical = std::fs::canonicalize(path)?;
    Utf8PathBuf::from_path_buf(canonical).map_err(|non_utf8_path| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "canonical fixture path is not valid UTF-8: {}",
                non_utf8_path.display()
            ),
        )
    })
}
