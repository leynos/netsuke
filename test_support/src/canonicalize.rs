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

#[cfg(test)]
mod tests {
    //! Regression coverage for fixture-path canonicalization.

    use super::canonicalize;
    use anyhow::{Context, Result, anyhow, ensure};
    use camino::{Utf8Path, Utf8PathBuf};
    use std::path::Path;
    use tempfile::tempdir;

    fn utf8_path(path: &Path) -> Result<&Utf8Path> {
        Utf8Path::from_path(path).context("fixture path must be valid UTF-8")
    }

    fn filesystem_canonical_path(path: &Path) -> Result<Utf8PathBuf> {
        let canonical = std::fs::canonicalize(path).context("canonicalize fixture path")?;
        Utf8PathBuf::from_path_buf(canonical).map_err(|resolved_path| {
            anyhow!("fixture canonical path must be valid UTF-8: {resolved_path:?}")
        })
    }

    #[test]
    fn canonicalize_resolves_dot_component_in_fixture_path() -> Result<()> {
        let temporary = tempdir().context("create temporary fixture directory")?;
        let fixture_directory = temporary.path().join("fixture");
        super::super::create_dir(&fixture_directory).context("create fixture directory")?;
        let fixture = fixture_directory.join("config.toml");
        super::super::write(&fixture, "jobs = 1\n").context("write fixture")?;

        let dotted_fixture = fixture_directory.join(".").join("config.toml");
        let canonical =
            canonicalize(utf8_path(&dotted_fixture)?).context("canonicalize fixture")?;
        let expected = filesystem_canonical_path(&fixture)?;
        ensure!(
            canonical == expected,
            "canonical fixture path {canonical} did not match filesystem spelling {expected}"
        );

        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn canonicalize_resolves_unix_symlink_alias_to_target() -> Result<()> {
        let temporary = tempdir().context("create temporary fixture directory")?;
        let target = temporary.path().join("target.toml");
        super::super::write(&target, "jobs = 1\n").context("write target fixture")?;
        let alias = temporary.path().join("alias.toml");
        super::super::symlink(&target, &alias).context("create fixture alias")?;

        let canonical = canonicalize(utf8_path(&alias)?).context("canonicalize alias")?;
        let expected = filesystem_canonical_path(&target)?;
        ensure!(
            canonical == expected,
            "canonical alias path {canonical} did not resolve to target {expected}"
        );

        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn canonicalize_rejects_non_utf8_resolved_unix_path() -> Result<()> {
        use std::ffi::OsString;
        use std::io::ErrorKind;
        use std::os::unix::ffi::OsStringExt;

        let temporary = tempdir().context("create temporary fixture directory")?;
        let non_utf8_name = OsString::from_vec(b"fixture-\xFF.toml".to_vec());
        let non_utf8_fixture = temporary.path().join(non_utf8_name);
        super::super::write(&non_utf8_fixture, "jobs = 1\n").context("write non-UTF-8 fixture")?;
        let utf8_alias = temporary.path().join("utf8-alias.toml");
        super::super::symlink(&non_utf8_fixture, &utf8_alias)
            .context("create UTF-8 alias to non-UTF-8 fixture")?;

        let error = canonicalize(utf8_path(&utf8_alias)?)
            .expect_err("a non-UTF-8 resolved path must be rejected");
        ensure!(
            error.kind() == ErrorKind::InvalidData,
            "expected InvalidData for non-UTF-8 resolved path, got {error}"
        );

        Ok(())
    }
}
