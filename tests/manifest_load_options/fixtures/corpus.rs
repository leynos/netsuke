//! Staging a corpus fixture in a temporary workspace.
//!
//! A path entry point loads a manifest from a workspace it opens itself, and a
//! manifest's supporting files have to be reachable from that workspace. The
//! fixtures live under `tests/data/`, so the workspace has to be a copy: the
//! loader refuses to leave the manifest's own directory, and that directory is
//! shared with every other suite in the repository.
//!
//! Copying the whole directory would be the obvious approach and is the wrong
//! one. `tests/data` holds manifests that call `shell` and `fetch`, and a
//! characterization run must not execute any of them as a side effect of
//! staging. Only the file and the siblings a fixture names are copied, so what a
//! row can reach is exactly what it declared.
//!
//! Relative structure is preserved rather than flattened. `glob()` resolves its
//! pattern against the manifest's own directory, so a fixture matching
//! `glob_files/*.txt` needs its matches one directory down; flattening them
//! beside the manifest would leave the row passing over an empty match set.

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};

use super::fixture::Case;

/// Hold a staged copy of a corpus fixture for the duration of one row.
///
/// Dropping the value removes the workspace. The staged manifest therefore has
/// a run-unique temporary path, which is why the rows filter it: without the
/// filter the snapshot would record the temporary directory's name and fail on
/// every run.
#[derive(Debug)]
pub struct StagedFixture {
    /// Owns the temporary directory; dropping it deletes the workspace.
    _tempdir: tempfile::TempDir,
    /// The staged manifest, ready to hand to a path entry point.
    manifest: Utf8PathBuf,
}

impl StagedFixture {
    /// Copy `case` and its declared siblings into a fresh temporary workspace.
    ///
    /// # Errors
    ///
    /// Returns an error when the workspace cannot be created, when the fixture
    /// or one of its siblings cannot be found, or when a copy fails.
    pub fn new(case: &Case) -> anyhow::Result<Self> {
        let manifest_relative = Utf8Path::new(case.path);
        let fixture_dir = manifest_relative
            .parent()
            .ok_or_else(|| anyhow::anyhow!("fixture path {} has no directory", case.path))?;
        let file_name = manifest_relative
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("fixture path {} has no file name", case.path))?;

        let tempdir = tempfile::tempdir()?;
        let workspace = Utf8Path::from_path(tempdir.path())
            .ok_or_else(|| anyhow::anyhow!("temporary directory path is not UTF-8"))?;
        let destination = Dir::open_ambient_dir(workspace, ambient_authority())?;
        let source = Dir::open_ambient_dir(fixture_dir, ambient_authority())?;
        source.copy(file_name, &destination, file_name)?;
        for sibling in case.siblings {
            copy_sibling(&source, &destination, fixture_dir, sibling)?;
        }
        let staged = tempdir.path().join(file_name);
        let manifest = Utf8PathBuf::from_path_buf(staged).map_err(|path| {
            anyhow::anyhow!("staged manifest path is not UTF-8: {}", path.display())
        })?;
        Ok(Self {
            manifest,
            _tempdir: tempdir,
        })
    }

    /// Return the staged manifest's path.
    pub fn manifest_path(&self) -> &Utf8Path {
        &self.manifest
    }
}

/// Copy one declared sibling into the workspace, preserving its relative path.
///
/// The sibling is named repository-relative, so it is made relative to the
/// fixture's own directory first; that is the path the staged manifest will
/// look for it at. The parent directories are created on the way, because the
/// copy is of a file, not of the tree that holds it.
///
/// # Errors
///
/// Returns an error when the sibling is not under the fixture's directory, when
/// an intermediate directory cannot be created, or when the copy fails.
fn copy_sibling(
    source: &Dir,
    destination: &Dir,
    fixture_dir: &Utf8Path,
    sibling: &str,
) -> anyhow::Result<()> {
    let relative = Utf8Path::new(sibling)
        .strip_prefix(fixture_dir)
        .map_err(|error| {
            anyhow::anyhow!("declared sibling {sibling} is not under {fixture_dir}: {error}")
        })?;
    for parent in relative.ancestors().skip(1) {
        if !parent.as_str().is_empty() {
            destination.create_dir_all(parent)?;
        }
    }
    source.copy(relative, destination, relative)?;
    Ok(())
}
