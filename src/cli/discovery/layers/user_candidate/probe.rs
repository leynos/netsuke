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
    //! The absence branch is covered through the discovery cases. These cases
    //! pin the three answers the production implementation maps on its own: a
    //! regular file is present, a link whose target is missing is a symlink, and
    //! an inspection that failed stays an error. The last one matters most
    //! because folding it into absence is the defect the probe exists to
    //! prevent, and it is the branch a real layout can stage here: the parent of
    //! the candidate is a regular file, so opening it fails outright.

    use super::{CandidateEntry, CandidateProbe, FsCandidateProbe, candidate_entry_metadata};
    use anyhow::{Context, Result, ensure};
    use rstest::rstest;
    use std::io;
    use std::path::Path;
    use tempfile::tempdir;

    /// Report a parent that is not a directory instead of treating it as absent.
    ///
    /// The loader also fails on this layout, so the case exercises the helper
    /// directly: what matters is that the helper does not answer `Ok(None)`,
    /// which is the value that would let discovery continue to a later scope.
    #[rstest]
    fn unopenable_parent_is_an_error_rather_than_an_absent_entry() -> Result<()> {
        let home = tempdir().context("create home directory")?;
        let candidate = stage_candidate_under_a_regular_file(home.path())?;

        let error = candidate_entry_metadata(&candidate)
            .err()
            .context("an unopenable parent must report an error, not absence")?;

        assert_not_a_directory(&error)
    }

    /// The production probe answers the same error the helper reports.
    ///
    /// This drives the trait method, not the helper: an implementation that
    /// swallowed the inspection failure into `Absent` would look correct
    /// through the helper alone.
    #[rstest]
    fn fs_probe_reports_an_unopenable_parent_as_an_error() -> Result<()> {
        let home = tempdir().context("create home directory")?;
        let candidate = stage_candidate_under_a_regular_file(home.path())?;

        let error = FsCandidateProbe
            .probe(&candidate)
            .err()
            .context("the production probe must report the failure, not absence")?;

        assert_not_a_directory(&error)
    }

    /// A real regular file is reported present.
    ///
    /// The mapping is not a tautology: collapsing the `Some(_)` arm into the
    /// `None` arm would report an installed candidate as absent and let
    /// discovery fall through to a lower scope.
    #[rstest]
    fn fs_probe_reports_a_regular_file_as_present() -> Result<()> {
        let home = tempdir().context("create home directory")?;
        let candidate = home.path().join("netsuke").join("config.toml");
        test_support::fs::create_dir_all(candidate.parent().context("candidate has a parent")?)
            .context("create the candidate's directory")?;
        test_support::fs::write(&candidate, "jobs = 2\n").context("write the candidate file")?;

        let entry = FsCandidateProbe
            .probe(&candidate)
            .context("a regular file is inspectable")?;

        ensure!(
            entry == CandidateEntry::Present,
            "a regular file must be reported present, got {entry:?}"
        );
        Ok(())
    }

    /// A link whose target is missing is reported as a symbolic link.
    ///
    /// `symlink_metadata` is what keeps this answer available: following the
    /// link would report absence instead, and the fail-closed rejection would
    /// never fire for the policy the user meant to override.
    #[rstest]
    fn fs_probe_reports_a_dangling_link_as_a_symlink() -> Result<()> {
        let home = tempdir().context("create home directory")?;
        let candidate = home.path().join(".netsuke.toml");
        test_support::fs::symlink(Path::new("missing.toml"), &candidate)
            .context("stage a link whose target does not exist")?;

        let entry = FsCandidateProbe
            .probe(&candidate)
            .context("a dangling link is still an inspectable entry")?;

        ensure!(
            entry == CandidateEntry::Symlink,
            "a link whose target is missing must be reported as a symlink, got {entry:?}"
        );
        Ok(())
    }

    /// Stage a candidate whose parent is a regular file rather than a directory.
    ///
    /// The caller keeps `home` alive: the staged parent lives inside it, so a
    /// returned path whose temporary directory had been dropped would name
    /// nothing and the probe would answer absence instead of failing.
    fn stage_candidate_under_a_regular_file(home: &Path) -> Result<std::path::PathBuf> {
        let blocking_parent = home.join(".config");
        test_support::fs::write(&blocking_parent, "not a directory\n")
            .context("stage a regular file where the XDG base expects a directory")?;
        Ok(blocking_parent.join("netsuke").join("config.toml"))
    }

    /// Require an inspection failure to keep the filesystem's own error kind.
    fn assert_not_a_directory(error: &io::Error) -> Result<()> {
        ensure!(
            error.kind() == io::ErrorKind::NotADirectory,
            "the inspection must keep the filesystem's own error kind, got {kind:?}",
            kind = error.kind()
        );
        Ok(())
    }
}
