//! Helpers for creating executable stubs in tests.
//!
//! These utilities write tiny shell/batch scripts and mark them executable so
//! tests can exercise PATH resolution without depending on real binaries.
//! Callers own the containing directory's lifetime to keep the stub on disk.
//!
//! Paths are camino UTF-8 types throughout, matching the rest of Netsuke.
//! `tempfile` yields OS-native paths, so callers convert at that boundary with
//! [`utf8_path`], which reports a non-UTF-8 path rather than discarding it.
//!
//! # Examples
//!
//! ```rust
//! use tempfile::TempDir;
//! use test_support::exec::{utf8_path, write_exec};
//!
//! let temp = TempDir::new().expect("tempdir");
//! let root = utf8_path(temp.path()).expect("temporary directory is UTF-8");
//! let path = write_exec(root, "tool").expect("stub executable");
//! assert!(path.exists());
//! ```

use crate::fs;
use anyhow::{Context, Result, bail};
use camino::{Utf8Component, Utf8Path, Utf8PathBuf};
use std::path::Path;

/// Borrow an OS-native path as UTF-8, naming the path when it is not.
///
/// This is the single conversion boundary between `tempfile`'s OS-native paths
/// and the camino types the stub helpers take. It returns an error rather than
/// panicking so callers propagate the failure with their own context.
///
/// # Errors
///
/// Returns an error identifying `path` when it is not valid UTF-8.
///
/// # Examples
///
/// ```rust
/// use tempfile::TempDir;
/// use test_support::exec::utf8_path;
///
/// let temp = TempDir::new().expect("tempdir");
/// let root = utf8_path(temp.path()).expect("temporary directory is UTF-8");
/// assert!(root.is_absolute());
/// ```
///
/// A path that is not valid UTF-8 is reported rather than silently lost:
///
/// ```rust
/// # #[cfg(unix)]
/// # {
/// use std::{ffi::OsString, os::unix::ffi::OsStringExt, path::PathBuf};
/// use test_support::exec::utf8_path;
///
/// let raw = PathBuf::from(OsString::from_vec(b"/tmp/not-\xff-utf8".to_vec()));
/// let err = utf8_path(&raw).expect_err("path is not UTF-8");
/// assert!(err.to_string().contains("not valid UTF-8"));
/// # }
/// ```
pub fn utf8_path(path: &Path) -> Result<&Utf8Path> {
    Utf8Path::from_path(path)
        .with_context(|| format!("path is not valid UTF-8: {}", path.display()))
}

/// Write a minimal executable file named `name` inside `root`.
///
/// # Errors
///
/// Returns an error when `name` is not a single file-name component, or
/// when the stub cannot be written or marked executable.
///
/// # Examples
///
/// ```rust
/// use tempfile::TempDir;
/// use test_support::exec::{utf8_path, write_exec};
///
/// let temp = TempDir::new().expect("tempdir");
/// let root = utf8_path(temp.path()).expect("temporary directory is UTF-8");
/// let path = write_exec(root, "tool").expect("stub executable");
/// assert!(test_support::fs::exists(&path));
/// # #[cfg(unix)]
/// # {
/// assert!(test_support::fs::is_executable_file(&path));
/// # }
/// ```
pub fn write_exec(root: &Utf8Path, name: &str) -> Result<Utf8PathBuf> {
    write_exec_with_content(root, name, "#!/bin/sh\n")
}

/// Write an executable script named `name` inside `root` with `content`.
///
/// This is the shared primitive behind the fake-executable helpers: it
/// creates the file, writes the script body verbatim, and marks the result
/// executable on Unix. Callers provide platform-appropriate content (for
/// example a POSIX shell script on Unix or a batch file on Windows).
///
/// # Errors
///
/// Returns an error when `name` is not a single file-name component, or
/// when the stub cannot be written or marked executable.
///
/// # Examples
///
/// ```rust
/// use tempfile::TempDir;
/// use test_support::exec::{utf8_path, write_exec_with_content};
///
/// let temp = TempDir::new().expect("tempdir");
/// let root = utf8_path(temp.path()).expect("temporary directory is UTF-8");
/// let path = write_exec_with_content(root, "tool", "#!/bin/sh\nexit 3\n")
///     .expect("stub executable");
/// assert!(test_support::fs::exists(&path));
/// ```
pub fn write_exec_with_content(root: &Utf8Path, name: &str, content: &str) -> Result<Utf8PathBuf> {
    single_file_name(name)?;
    let path = root.join(name);
    fs::write(&path, content).with_context(|| format!("write exec stub {name}"))?;
    make_executable(&path)?;
    Ok(path)
}

/// Reject a stub name that is anything but a single file-name component.
///
/// The stub writers join `name` onto `root`, so a name carrying a separator, a
/// parent component, or a root would put the stub somewhere other than the
/// caller's temporary directory — silently, because the write would still
/// succeed. Every caller passes a literal today, which makes this a property of
/// the call sites; checking here makes it a property of the helper instead.
///
/// The component must equal `name` outright, which also rejects a trailing
/// separator: `components()` normalises `"tool/"` to a lone `Normal("tool")`,
/// so comparing against the original string is what catches it.
fn single_file_name(name: &str) -> Result<()> {
    let mut components = Utf8Path::new(name).components();
    match (components.next(), components.next()) {
        (Some(Utf8Component::Normal(component)), None) if component == name => Ok(()),
        _ => bail!("exec stub name must be a single file name, got {name:?}"),
    }
}

/// Mark an existing file as executable by setting its Unix permission bits.
///
/// # Errors
///
/// Returns an error when the permission bits cannot be set.
///
/// # Examples
///
/// ```rust
/// # #[cfg(unix)]
/// # {
/// use tempfile::TempDir;
/// use test_support::exec::{make_executable, utf8_path};
/// use test_support::fs::is_executable_file;
///
/// let temp = TempDir::new().expect("tempdir");
/// let root = utf8_path(temp.path()).expect("temporary directory is UTF-8");
/// let path = root.join("tool");
/// test_support::fs::write(&path, "#!/bin/sh\n").expect("write stub");
/// assert!(!is_executable_file(&path));
/// make_executable(&path).expect("mark executable");
/// assert!(is_executable_file(&path));
/// # }
/// ```
#[cfg(unix)]
pub fn make_executable(path: &Utf8Path) -> Result<()> {
    fs::set_mode(path, 0o755).context("chmod exec stub")?;
    Ok(())
}

/// No-op on non-Unix platforms, where executability is not a permission bit.
///
/// # Errors
///
/// Never returns an error; the signature matches the Unix variant.
#[cfg(not(unix))]
pub fn make_executable(_path: &Utf8Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Coverage for the stub-name guard.
    //!
    //! The guard exists so a name can never place the stub outside the root it
    //! was given, so the rejected cases are the interesting ones.

    use super::{single_file_name, write_exec_with_content};
    use crate::exec::utf8_path;
    use rstest::rstest;
    use tempfile::TempDir;

    #[rstest]
    #[case::plain("tool")]
    #[case::dotted("ninja.cmd")]
    #[case::leading_dot(".hidden")]
    fn accepts_a_single_file_name(#[case] name: &str) {
        assert!(
            single_file_name(name).is_ok(),
            "{name:?} should be accepted"
        );
    }

    #[rstest]
    #[case::empty("")]
    #[case::current_dir(".")]
    #[case::parent("..")]
    #[case::separator("nested/tool")]
    #[case::leading_separator("/tool")]
    #[case::traversal("../tool")]
    #[case::dot_prefixed("./tool")]
    #[case::trailing_separator("tool/")]
    fn rejects_anything_else(#[case] name: &str) {
        assert!(
            single_file_name(name).is_err(),
            "{name:?} should be rejected"
        );
    }

    #[test]
    fn write_exec_with_content_rejects_a_traversing_name() {
        let temp = TempDir::new().expect("tempdir");
        let root = utf8_path(temp.path()).expect("temporary directory is UTF-8");
        let error = write_exec_with_content(root, "../escaped", "#!/bin/sh\n")
            .expect_err("a traversing name should be rejected");
        assert!(
            format!("{error:#}").contains("single file name"),
            "unexpected error: {error:#}"
        );
    }
}
