//! Establishment of the generated dyndep directory through verified handles.
//!
//! `.netsuke` and `.netsuke/dyndep` are project-controlled state: a manifest
//! can ship either component as a symbolic link or a Windows junction, and a
//! capability rooted at the workspace would otherwise resolve that alias. A
//! cleanup pass that followed the alias would treat unrelated workspace files
//! as disposable cache entries and delete them.
//!
//! Every dyndep operation therefore walks the two components one at a time,
//! refuses any entry that is not a real directory, and keeps the resulting
//! handle for publication, locking, and retention. The refusal is checked
//! twice: once on the entry before it is opened, so the diagnostic names the
//! aliased component, and once on the opened handle, so an entry swapped in
//! between the two cannot smuggle an alias past the check. Unix opens with
//! `O_NOFOLLOW`; Windows opens the reparse point itself and rejects every
//! reparse attribute, not only the tags `std` reports as symlinks.

use super::super::DYNDEP_DIR;
use crate::localization::{self, keys};
use anyhow::{Context, Result, anyhow};
use camino::{Utf8Path, Utf8PathBuf};
use cap_primitives::fs::open_dir_nofollow;
use cap_std::fs_utf8::{Dir, Metadata};
use std::io::ErrorKind;

/// Components of the generated dyndep namespace, in walk order.
const DYNDEP_COMPONENTS: [&str; 2] = [".netsuke", "dyndep"];

/// A verified handle to the generated dyndep directory.
///
/// The handle is the only authority any dyndep operation may use: every path
/// beneath it is a bare entry name, so no operation can resolve a component
/// outside the directory this type verified.
pub(crate) struct DyndepDirectory {
    /// Verified directory handle for `.netsuke/dyndep`.
    dir: Dir,
}

impl DyndepDirectory {
    /// Open the generated dyndep directory, creating missing components.
    ///
    /// # Errors
    ///
    /// Returns an error when a component cannot be created or opened, or when
    /// either component is a symbolic link, junction, or non-directory.
    pub(crate) fn open_or_create(workspace: &Dir) -> Result<Self> {
        let mut current = clone_handle(workspace, Utf8Path::new("."))?;
        let mut display = Utf8PathBuf::new();
        for component in DYNDEP_COMPONENTS {
            display.push(component);
            current = match open_component(&current, component, &display)? {
                Some(dir) => dir,
                None => create_component(&current, component, &display)?,
            };
        }
        Ok(Self { dir: current })
    }

    /// Open the generated dyndep directory, reporting absence as `None`.
    ///
    /// # Errors
    ///
    /// Returns an error when a component cannot be opened, or when either
    /// component is a symbolic link, junction, or non-directory.
    pub(crate) fn open_existing(workspace: &Dir) -> Result<Option<Self>> {
        let mut current = clone_handle(workspace, Utf8Path::new("."))?;
        let mut display = Utf8PathBuf::new();
        for component in DYNDEP_COMPONENTS {
            display.push(component);
            let Some(dir) = open_component(&current, component, &display)? else {
                return Ok(None);
            };
            current = dir;
        }
        Ok(Some(Self { dir: current }))
    }

    /// Borrow the verified directory handle.
    pub(crate) const fn dir(&self) -> &Dir {
        &self.dir
    }
}

/// Whether `metadata` describes a symbolic link, junction, or other reparse
/// point.
///
/// Windows tests the reparse attribute rather than the tag value, so a
/// deduplication or cloud placeholder is refused along with a symlink or
/// junction. Unix has no other alias kind, so the file type is decisive.
pub(crate) fn is_alias(metadata: &Metadata) -> bool {
    #[cfg(windows)]
    {
        use cap_std::fs::MetadataExt as _;
        crate::stdlib::path::windows_reparse::is_prohibited_reparse_point(
            metadata.file_attributes(),
        )
    }
    #[cfg(not(windows))]
    {
        metadata.is_symlink()
    }
}

/// Whether `metadata` is anything other than a real directory.
fn rejects_component(metadata: &Metadata) -> bool {
    is_alias(metadata) || !metadata.is_dir()
}

/// Open one component without following an alias, reporting absence as `None`.
fn open_component(parent: &Dir, name: &str, display: &Utf8Path) -> Result<Option<Dir>> {
    let metadata = match parent.symlink_metadata(name) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).with_context(|| open_error(display)),
    };
    if rejects_component(&metadata) {
        return Err(anyhow!(alias_error(display)));
    }
    let opened = open_nofollow(parent, name).with_context(|| open_error(display))?;
    // The entry could have been replaced between the inspection and the open.
    // Judging the opened handle closes that window: the handle is what every
    // later operation uses, so it is the only metadata that matters.
    let opened_metadata = opened.dir_metadata().with_context(|| open_error(display))?;
    if rejects_component(&opened_metadata) {
        return Err(anyhow!(alias_error(display)));
    }
    Ok(Some(opened))
}

/// Create one missing component and open the result without following aliases.
fn create_component(parent: &Dir, name: &str, display: &Utf8Path) -> Result<Dir> {
    match parent.create_dir(name) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error).with_context(|| create_error(display)),
    }
    match open_component(parent, name, display)? {
        Some(dir) => Ok(dir),
        // A concurrent remover can delete the component between the create and
        // the open. Reporting the open failure is honest: the directory this
        // call promised to establish is not there.
        None => Err(anyhow!(open_error(display))),
    }
}

/// Open `name` beneath `parent` without following a final-component alias.
fn open_nofollow(parent: &Dir, name: &str) -> std::io::Result<Dir> {
    let parent_file = parent.as_ref().try_clone()?.into_std_file();
    open_dir_nofollow(&parent_file, Utf8Path::new(name).as_std_path()).map(Dir::from_std_file)
}

/// Clone a directory handle, attaching the localized open diagnostic.
fn clone_handle(dir: &Dir, display: &Utf8Path) -> Result<Dir> {
    dir.try_clone().with_context(|| open_error(display))
}

/// Build the localized alias diagnostic for a dyndep `path`.
fn alias_error(path: &Utf8Path) -> crate::localization::LocalizedMessage {
    localization::message(keys::RUNNER_IO_DYNDEP_ALIAS).with_arg("path", path.as_str())
}

/// Build the localized open diagnostic for a dyndep `path`.
fn open_error(path: &Utf8Path) -> crate::localization::LocalizedMessage {
    localization::message(keys::RUNNER_IO_DYNDEP_OPEN).with_arg("path", path.as_str())
}

/// Build the localized create diagnostic for a dyndep `path`.
fn create_error(path: &Utf8Path) -> crate::localization::LocalizedMessage {
    localization::message(keys::RUNNER_IO_DYNDEP_CREATE_DIR).with_arg("path", path.as_str())
}

/// Return the workspace-relative display path of a dyndep entry.
pub(crate) fn entry_display_path(name: &str) -> Utf8PathBuf {
    Utf8Path::new(DYNDEP_DIR).join(name)
}

#[cfg(test)]
mod tests;
