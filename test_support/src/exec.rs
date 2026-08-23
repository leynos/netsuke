//! Helpers for creating executable stubs in tests.
//!
//! These utilities write tiny shell/batch scripts and mark them executable so
//! tests can exercise PATH resolution without depending on real binaries.
//! Callers own the containing directory's lifetime to keep the stub on disk.
//!
//! # Examples
//!
//! ```rust
//! use tempfile::TempDir;
//! use test_support::write_exec;
//!
//! let temp = TempDir::new().expect("tempdir");
//! let path = write_exec(temp.path(), "tool").expect("stub executable");
//! assert!(path.exists());
//! ```

use anyhow::{Context, Result, bail};
use std::{
    ffi::OsStr,
    path::{Component, Path, PathBuf},
};

/// Write a minimal executable file named `name` inside `root`.
///
/// # Errors
///
/// Returns an error if the executable cannot be written or made executable.
pub fn write_exec(root: &Path, name: &str) -> Result<PathBuf> {
    write_exec_with_content(root, name, "#!/bin/sh\n")
}

/// Write an executable script named `name` inside `root` with `content`.
///
/// This is the shared primitive behind the fake-executable helpers: it
/// creates the file, writes the script body verbatim, and marks the result
/// executable on Unix. Callers provide platform-appropriate content (for
/// example a POSIX shell script on Unix or a batch file on Windows).
///
/// # Examples
///
/// ```rust
/// use tempfile::TempDir;
/// use test_support::exec::write_exec_with_content;
///
/// let temp = TempDir::new().expect("tempdir");
/// let path = write_exec_with_content(temp.path(), "tool", "#!/bin/sh\nexit 3\n")
///     .expect("stub executable");
/// assert!(path.exists());
/// ```
///
/// # Errors
///
/// Returns an error if the executable cannot be written or made executable.
pub fn write_exec_with_content(root: &Path, name: &str, content: &str) -> Result<PathBuf> {
    single_file_name(name)?;
    let path = root.join(name);
    crate::fs::write(&path, content).with_context(|| format!("write exec stub {name}"))?;
    make_executable(&path)?;
    Ok(path)
}

/// Reject a stub name that is not exactly one normal path component.
fn single_file_name(name: &str) -> Result<()> {
    let mut components = Path::new(name).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(component)), None) if component == OsStr::new(name) => Ok(()),
        _ => bail!("exec stub name must be a single file name, got {name:?}"),
    }
}

/// Mark an existing file as executable by setting its Unix permission bits.
///
/// # Errors
///
/// Returns an error if executable permissions cannot be applied to the path.
#[cfg(unix)]
pub fn make_executable(path: &Path) -> Result<()> {
    crate::fs::set_mode(path, 0o755).context("chmod exec stub")?;
    Ok(())
}

/// No-op on non-Unix platforms, where executability is not a permission bit.
///
/// # Errors
///
/// Never returns an error; the fallible signature matches the Unix variant so
/// callers need no platform-specific handling.
#[cfg(not(unix))]
pub const fn make_executable(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Coverage for the executable-stub name boundary.

    use super::single_file_name;
    use rstest::rstest;

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
}
