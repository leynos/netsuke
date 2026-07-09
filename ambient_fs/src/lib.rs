//! Deliberately ambient filesystem probes for executable lookup.
//!
//! Netsuke routes filesystem access through capability-scoped `cap_std`
//! handles, enforced by Whitaker's `no_std_fs_operations` lint. The `which`
//! resolver is the one deliberate exception: it consults directories taken
//! from the ambient `PATH` environment variable and canonicalises candidates
//! across arbitrary symlinks, which capability-scoped handles cannot express
//! (a `cap_std` directory handle refuses to follow a symlink that leaves the
//! directory, breaking common layouts such as `/usr/bin/cc ->
//! /etc/alternatives/cc`).
//!
//! This crate confines those ambient operations to a single leaf crate that
//! is excluded from the lint via `dylint.toml`, mirroring the pattern used by
//! other df12 projects. Scope and reuse policy: only the `which` resolver and
//! runner tempfile plumbing may call into this crate; new ambient access
//! elsewhere should use `cap_std` instead.

use std::{fs, io};

use camino::Utf8Path;

/// Return `true` when `path` names an executable regular file.
///
/// Follows symlinks, matching `std::fs::metadata` semantics. On Unix an
/// executable file needs at least one execute permission bit; on other
/// platforms any regular file qualifies.
///
/// # Errors
///
/// Propagates metadata errors, including `NotFound` when the path is absent;
/// callers decide whether absence is an error.
///
/// # Examples
///
/// ```
/// use camino::Utf8Path;
///
/// let sh = Utf8Path::new("/bin/sh");
/// if let Ok(executable) = ambient_fs::is_executable_file(sh) {
///     assert!(executable, "/bin/sh should be executable where it exists");
/// }
/// ```
pub fn is_executable_file(path: &Utf8Path) -> io::Result<bool> {
    let metadata = fs::metadata(path.as_std_path())?;
    Ok(metadata.is_file() && has_execute_permission(&metadata))
}

#[cfg(unix)]
fn has_execute_permission(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn has_execute_permission(metadata: &fs::Metadata) -> bool {
    metadata.is_file()
}

/// Canonicalise `path`, resolving symlinks across the whole filesystem.
///
/// Returns the canonical path as a raw `std::path::PathBuf` so callers keep
/// control over UTF-8 validation and its error mapping.
///
/// # Errors
///
/// Propagates the underlying `canonicalize` failure (missing path, permission
/// denied, and so on).
///
/// # Examples
///
/// ```
/// use camino::Utf8Path;
///
/// let canonical = ambient_fs::canonicalize(Utf8Path::new(".")).expect("cwd resolves");
/// assert!(canonical.is_absolute());
/// ```
pub fn canonicalize(path: &Utf8Path) -> io::Result<std::path::PathBuf> {
    fs::canonicalize(path.as_std_path())
}

/// Return `true` when a walk entry refers to a regular file.
///
/// # Examples
///
/// ```
/// let mut walker = walkdir::WalkDir::new(".").max_depth(1).into_iter();
/// let root = walker.next().expect("walk yields the root").expect("root is readable");
/// assert!(!ambient_fs::entry_is_file(&root), "the root directory is not a file");
/// ```
#[must_use]
pub fn entry_is_file(entry: &walkdir::DirEntry) -> bool {
    entry.file_type().is_file()
}

/// Return `true` when a walk entry refers to a directory.
///
/// # Examples
///
/// ```
/// let mut walker = walkdir::WalkDir::new(".").max_depth(1).into_iter();
/// let root = walker.next().expect("walk yields the root").expect("root is readable");
/// assert!(ambient_fs::entry_is_dir(&root), "the walk root is a directory");
/// ```
#[must_use]
pub fn entry_is_dir(entry: &walkdir::DirEntry) -> bool {
    entry.file_type().is_dir()
}

/// Flush a file's contents and metadata to disk via its open handle.
///
/// Used for tempfiles that live in the ambient system temp directory, where
/// no capability-scoped directory handle exists to sync through.
///
/// # Errors
///
/// Propagates the underlying `sync_all` failure.
///
/// # Examples
///
/// ```
/// let file = tempfile::tempfile().expect("create tempfile");
/// ambient_fs::sync_file(&file).expect("sync tempfile");
/// ```
pub fn sync_file(file: &fs::File) -> io::Result<()> {
    file.sync_all()
}

#[cfg(test)]
mod tests {
    //! Unit tests for the ambient filesystem probes.
    use super::{canonicalize, is_executable_file, sync_file};
    use camino::{Utf8Path, Utf8PathBuf};
    use rstest::{fixture, rstest};
    use std::fs;
    use std::io;

    /// Assert that probing `path` with `is_executable_file` yields
    /// `expected`.
    ///
    /// Splits the fallible probe (the command) from the assertion on its
    /// result (the query) so an I/O failure reads as a probe error, not a
    /// wrong answer; a macro rather than a helper fn keeps panic line
    /// numbers pointing at the calling test.
    macro_rules! assert_executable_probe {
        ($path:expr, $expected:expr) => {
            let probed = is_executable_file($path).expect("probe executable state");
            assert_eq!(
                probed, $expected,
                "unexpected executable probe for {}",
                $path
            );
        };
    }

    /// Create a temporary directory and its owned UTF-8 path. The `TempDir` is
    /// returned so the caller keeps it alive for the directory's lifetime.
    ///
    /// Returns a `Result` because fixtures arrange state rather than assert
    /// on it: arrangement failures must propagate to the test body, which is
    /// the only place a panic is a test verdict (and the only context
    /// Whitaker's `no_expect_outside_tests` recognises).
    #[fixture]
    fn temp_dir() -> io::Result<(tempfile::TempDir, Utf8PathBuf)> {
        let dir = tempfile::tempdir()?;
        let path = Utf8Path::from_path(dir.path())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "temp path is not UTF-8"))?
            .to_owned();
        Ok((dir, path))
    }

    #[cfg(unix)]
    #[rstest]
    #[case(0o755, true)]
    #[case(0o644, false)]
    fn is_executable_file_reflects_unix_execute_bits(#[case] mode: u32, #[case] expected: bool) {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("create tempdir");
        let path = dir.path().join("candidate");
        fs::write(&path, b"").expect("write candidate file");
        let mut perms = fs::metadata(&path).expect("read metadata").permissions();
        perms.set_mode(mode);
        fs::set_permissions(&path, perms).expect("set permissions");
        let utf8 = Utf8Path::from_path(&path).expect("temp path is UTF-8");
        assert_executable_probe!(utf8, expected);
    }

    #[cfg(not(unix))]
    #[rstest]
    fn is_executable_file_accepts_any_regular_file() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let path = dir.path().join("candidate");
        fs::write(&path, b"").expect("write candidate file");
        let utf8 = Utf8Path::from_path(&path).expect("temp path is UTF-8");
        assert_executable_probe!(utf8, true);
    }

    #[rstest]
    fn is_executable_file_rejects_directories(
        temp_dir: io::Result<(tempfile::TempDir, Utf8PathBuf)>,
    ) -> io::Result<()> {
        let (_dir, utf8) = temp_dir?;
        assert_executable_probe!(&utf8, false);
        Ok(())
    }

    #[rstest]
    fn is_executable_file_reports_missing_paths(
        temp_dir: io::Result<(tempfile::TempDir, Utf8PathBuf)>,
    ) -> io::Result<()> {
        let (_dir, dir_path) = temp_dir?;
        let missing = dir_path.join("does-not-exist");
        let err = is_executable_file(&missing).expect_err("missing path should error");
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
        Ok(())
    }

    #[rstest]
    fn canonicalize_resolves_relative_traversal(
        temp_dir: io::Result<(tempfile::TempDir, Utf8PathBuf)>,
    ) -> io::Result<()> {
        // Force real filesystem resolution: a `..` round-trip only collapses
        // back to the original directory if `canonicalize` consults the
        // filesystem rather than returning its input unchanged. `std::fs`
        // canonicalization of the un-traversed path is the oracle.
        let (_dir, root) = temp_dir?;
        let nested = root.join("nested");
        fs::create_dir(nested.as_std_path()).expect("create nested dir");
        let traversed = nested.join("..").join("nested");
        let resolved = canonicalize(&traversed).expect("canonicalize traversal");
        let expected = fs::canonicalize(nested.as_std_path()).expect("canonicalize nested");
        assert_eq!(resolved, expected);
        Ok(())
    }

    #[rstest]
    fn sync_file_persists_written_bytes(
        temp_dir: io::Result<(tempfile::TempDir, Utf8PathBuf)>,
    ) -> io::Result<()> {
        // Behavioural oracle: sync then reopen by path and read back, proving
        // the bytes are persisted rather than merely that `sync_all` returned
        // `Ok`. (True crash durability cannot be verified without OS-level
        // fault injection, which is out of scope here.)
        use std::io::Write as _;
        let (_dir, root) = temp_dir?;
        let path = root.join("synced");
        let payload = b"ambient_fs sync payload".to_vec();
        let mut file = fs::File::create(path.as_std_path()).expect("create file for sync");
        file.write_all(&payload).expect("write payload");
        sync_file(&file).expect("sync file");
        let read_back = fs::read(path.as_std_path()).expect("read synced file");
        assert_eq!(read_back, payload);
        Ok(())
    }
}
