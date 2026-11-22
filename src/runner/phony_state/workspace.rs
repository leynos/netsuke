//! The working directory a build resolved once, as a path and a handle.
//!
//! The reservation check and the force-node reset must agree on which
//! directory they are talking about. Resolving it once and passing the result
//! to both removes the chance that a second lookup answers differently, and it
//! gives tests a way to inject a directory without building the application
//! configuration.

use super::super::path_helpers::selected_directory;
use crate::cli::Cli;
use crate::localization::{self, keys};
use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};

/// One working directory, resolved to a canonical path and an open handle.
///
/// Reading and writing both use the handle, so no operation climbs out of the
/// directory the canonical path names. The path is kept because the
/// reservation rule compares spellings against it.
pub(in crate::runner) struct Workspace {
    /// Absolute path with every link on the way resolved.
    canonical: Utf8PathBuf,
    /// Capability handle rooted at the same directory.
    dir: Dir,
}

impl Workspace {
    /// Open and canonicalize one directory as a build workspace.
    ///
    /// # Errors
    ///
    /// Returns an error when the directory does not exist, cannot be read,
    /// resolves to a path that is not valid UTF-8, or cannot be opened.
    pub(in crate::runner) fn open(root: &Utf8Path) -> Result<Self> {
        let canonical = root
            .canonicalize_utf8()
            .with_context(|| open_context(root))?;
        let dir = Dir::open_ambient_dir(&canonical, ambient_authority())
            .with_context(|| open_context(root))?;
        Ok(Self { canonical, dir })
    }

    /// Resolve the working directory the command line selected.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Self::open`].
    pub(in crate::runner) fn resolve(cli: &Cli) -> Result<Self> {
        Self::open(&selected_directory(cli))
    }

    /// Return the canonical path every spelling is judged against.
    pub(in crate::runner) fn canonical(&self) -> &Utf8Path {
        &self.canonical
    }

    /// Return the capability handle rooted at this workspace.
    pub(super) const fn dir(&self) -> &Dir {
        &self.dir
    }
}

/// Build the context for a workspace that cannot be resolved or opened.
fn open_context(path: &Utf8Path) -> localization::LocalizedMessage {
    localization::message(keys::RUNNER_IO_OPEN_AMBIENT_DIR).with_arg("path", path.as_str())
}
