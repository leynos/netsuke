//! Presence probe that separates an absent user candidate from a broken one.
//!
//! `OrthoConfig` resolves a candidate through a capability on its parent
//! directory and reports a link whose target does not resolve exactly as it
//! reports a file that is not installed. The user pass must tell those apart,
//! because falling through to the next location would hand the policy the user
//! meant to override back to system scope. [`CandidateProbe`] is that second
//! look, [`FsCandidateProbe`] is the production implementation, and
//! [`candidate_entry_metadata`] is the `cap_std` inspection behind it.
//!
//! A real directory can produce three of the four answers deterministically:
//! absent, present, and a dangling symbolic link. The fourth — an inspection
//! that fails — is a race against the loader's own view of the same directory,
//! because the loader opens that parent first and reports its failure before
//! this probe runs. Tests therefore drive that answer through the trait, which
//! is the reason the probe is a seam rather than a free function call.

use cap_std::ambient_authority;
use cap_std::fs::{Dir, Metadata};
use std::io;
use std::path::Path;

/// What a [`CandidateProbe`] found at a user candidate path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::cli::discovery) enum CandidateEntry {
    /// Nothing exists at the path.
    Absent,
    /// The entry is a symbolic link whose target did not resolve.
    Symlink,
    /// An entry exists and is not a symbolic link.
    Present,
}

/// Presence probe for one user-scope candidate.
///
/// The loader reports an unreadable candidate exactly as it reports an absent
/// one, so the fail-closed rule needs a second look at the directory entry
/// itself. Keeping that look behind a trait lets a test drive the three
/// answers, and the probe's own failure, without staging a directory that
/// happens to break the loader first.
pub(in crate::cli::discovery) trait CandidateProbe {
    /// Report what the filesystem holds at `path`.
    ///
    /// # Errors
    ///
    /// Returns the inspection error when the entry cannot be examined, for
    /// example because a parent component is not a directory.
    fn probe(&self, path: &Path) -> io::Result<CandidateEntry>;
}

/// The filesystem-backed probe production discovery uses.
pub(in crate::cli::discovery) struct FsCandidateProbe;

impl CandidateProbe for FsCandidateProbe {
    fn probe(&self, path: &Path) -> io::Result<CandidateEntry> {
        candidate_entry_metadata(path).map(|found| match found {
            Some(metadata) if metadata.file_type().is_symlink() => CandidateEntry::Symlink,
            Some(_) => CandidateEntry::Present,
            None => CandidateEntry::Absent,
        })
    }
}

/// Inspect a candidate entry without following a link at its final component.
///
/// The probe holds only the candidate's parent directory, so it reads no more
/// authority than the discovery loader that reports an unresolvable candidate
/// as absent. A missing entry, or a missing parent directory, reports absence.
/// Any other failure is returned rather than folded into that answer: a
/// parent that is not a directory, or one the process may not open, is a
/// different fact from an uninstalled file, and reporting it as absence would
/// silently fall through to the next scope.
///
/// # Errors
///
/// Returns an I/O error when the parent directory cannot be opened or the entry
/// cannot be inspected.
fn candidate_entry_metadata(path: &Path) -> io::Result<Option<Metadata>> {
    let Some(name) = path.file_name() else {
        return Ok(None);
    };
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let dir = match Dir::open_ambient_dir(parent, ambient_authority()) {
        Ok(dir) => dir,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    match dir.symlink_metadata(name) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    //! Production-probe tests a real directory can express.
    //!
    //! The absence branches are covered through the discovery cases. These
    //! cases pin the opposite answer — an inspection that failed — because
    //! folding it into absence is the defect the probe exists to prevent, and
    //! it is the one branch the loader cannot report first here: the parent of
    //! the candidate is a regular file, so opening it fails outright.

    use super::candidate_entry_metadata;
    use anyhow::{Context, Result, ensure};
    use rstest::rstest;
    use std::io;
    use tempfile::tempdir;

    /// Report a parent that is not a directory instead of treating it as absent.
    ///
    /// The loader also fails on this layout, so the case exercises the helper
    /// directly: what matters is that the helper does not answer `Ok(None)`,
    /// which is the value that would let discovery continue to a later scope.
    #[rstest]
    fn unopenable_parent_is_an_error_rather_than_an_absent_entry() -> Result<()> {
        let home = tempdir().context("create home directory")?;
        let blocking_parent = home.path().join(".config");
        test_support::fs::write(&blocking_parent, "not a directory\n")
            .context("stage a regular file where the XDG base expects a directory")?;

        let candidate = blocking_parent.join("netsuke").join("config.toml");
        let error = candidate_entry_metadata(&candidate)
            .err()
            .context("an unopenable parent must report an error, not absence")?;

        ensure!(
            error.kind() == io::ErrorKind::NotADirectory,
            "the inspection must keep the filesystem's own error kind, got {kind:?}",
            kind = error.kind()
        );
        Ok(())
    }
}
