//! Generated dyndep bundle values shared by rendering and publication.
//!
//! These immutable values mark the boundary between the generator's query and
//! the runner's publication command. The generator alone chooses their paths
//! and content; the runner may only borrow or consume them for materialization.

use camino::Utf8PathBuf;

/// One generated dyndep sidecar file inside a [`GeneratedNinja`] bundle.
///
/// `relative_path` is relative to the effective Ninja working directory and
/// matches the path the main build file references. `content` is the full
/// Ninja-syntax dyndep document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedDyndep {
    pub(super) relative_path: Utf8PathBuf,
    pub(super) content: String,
}

impl GeneratedDyndep {
    /// Borrow the sidecar path relative to the effective Ninja working
    /// directory.
    #[must_use]
    pub const fn relative_path(&self) -> &Utf8PathBuf {
        &self.relative_path
    }

    /// Borrow the dyndep document content to materialize.
    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
    }
}

/// The complete generated Ninja artefact: the main build file text plus every
/// dyndep sidecar required to load and execute it.
///
/// All paths are relative to the effective Ninja working directory. Do not
/// invoke Ninja on [`GeneratedNinja::build_file`] until every sidecar in
/// [`GeneratedNinja::dyndep_files`] has been materialized beside it.
#[derive(Debug, Clone)]
pub struct GeneratedNinja {
    pub(super) build_file: String,
    pub(super) dyndep_files: Vec<GeneratedDyndep>,
}

impl GeneratedNinja {
    /// Borrow the main Ninja build file text.
    #[must_use]
    pub fn build_file(&self) -> &str {
        &self.build_file
    }

    /// Borrow the dyndep sidecars required by `build_file`.
    #[must_use]
    pub fn dyndep_files(&self) -> &[GeneratedDyndep] {
        &self.dyndep_files
    }

    /// Consume the bundle, returning the main file text and its sidecars.
    #[must_use]
    pub fn into_parts(self) -> (String, Vec<GeneratedDyndep>) {
        (self.build_file, self.dyndep_files)
    }
}

#[cfg(test)]
impl GeneratedDyndep {
    /// Build a sidecar fixture for tests that must construct bundles from
    /// scratch rather than through generation.
    #[must_use]
    pub(crate) fn fixture(relative_path: Utf8PathBuf, content: String) -> Self {
        Self {
            relative_path,
            content,
        }
    }
}
