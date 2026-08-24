//! UTF-8 file-system helpers for stdlib filters using cap-std Dir handles: metadata queries,
//! opening files for streaming, and safe error translation.
use std::io;

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{
    ambient_authority, fs,
    fs_utf8::{Dir, File, OpenOptions},
};
use minijinja::Error;

use crate::localization::{self, keys};

use super::path_utils::normalise_parent;
use crate::stdlib::io_helpers::io_to_error;

/// An ambient handle to a path's parent directory and the entry name within it.
pub(super) struct ParentDir {
    /// Capability-scoped handle to the parent directory.
    pub handle: Dir,
    /// The final path component, addressed within `handle`.
    pub entry: String,
    /// The parent directory's own path.
    pub dir_path: Utf8PathBuf,
}

/// Open a path's parent directory with ambient authority.
///
/// # Errors
///
/// Returns the underlying I/O error when the parent directory cannot be
/// opened.
pub(super) fn parent_dir(path: &Utf8Path) -> Result<ParentDir, io::Error> {
    let dir_path = normalise_parent(path.parent());
    let handle = Dir::open_ambient_dir(&dir_path, ambient_authority())?;
    let entry = path.file_name().map_or_else(|| ".".into(), str::to_owned);
    Ok(ParentDir {
        handle,
        entry,
        dir_path,
    })
}

/// Open a path's parent directory, mapping failures to template errors.
///
/// # Errors
///
/// Returns a template error when the parent directory cannot be opened.
pub(super) fn open_parent_dir(path: &Utf8Path) -> Result<ParentDir, Error> {
    parent_dir(path).map_err(|err| {
        io_to_error(
            path,
            &localization::message(keys::STDLIB_PATH_ACTION_OPEN_DIRECTORY),
            err,
        )
    })
}

/// Execute an operation on a file's parent directory handle, translating I/O errors
/// with the appropriate localized action message.
fn with_parent_dir<T, F>(
    path: &Utf8Path,
    action_key: &'static str,
    operation: F,
) -> Result<T, Error>
where
    F: FnOnce(&Dir, &str) -> io::Result<T>,
{
    let parent = open_parent_dir(path)?;
    operation(&parent.handle, &parent.entry)
        .map_err(|err| io_to_error(path, &localization::message(action_key), err))
}

/// Determine whether the path's file type satisfies `predicate`, counting missing paths as non-matching.
///
/// # Errors
///
/// Returns a template error when the parent directory or metadata cannot be
/// read for a reason other than the path being absent.
pub(crate) fn file_type_matches<F>(path: &Utf8Path, predicate: F) -> Result<bool, Error>
where
    F: Fn(fs::FileType) -> bool,
{
    match parent_dir(path) {
        Ok(parent) => match parent.handle.symlink_metadata(Utf8Path::new(&parent.entry)) {
            Ok(metadata) => Ok(predicate(metadata.file_type())),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(err) => Err(io_to_error(
                path,
                &localization::message(keys::STDLIB_PATH_ACTION_STAT),
                err,
            )),
        },
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(io_to_error(
            path,
            &localization::message(keys::STDLIB_PATH_ACTION_OPEN_DIRECTORY),
            err,
        )),
    }
}

/// Return the byte length of the file at `path`.
///
/// # Errors
///
/// Returns a template error when the parent directory cannot be opened or the
/// file metadata cannot be read.
pub(super) fn file_size(path: &Utf8Path) -> Result<u64, Error> {
    with_parent_dir(path, keys::STDLIB_PATH_ACTION_STAT, |handle, entry| {
        handle
            .metadata(Utf8Path::new(entry))
            .map(|metadata| metadata.len())
    })
}

/// Read the file at `path` as UTF-8 text.
///
/// # Errors
///
/// Returns a template error when the parent directory cannot be opened, the
/// file cannot be read, or its contents are not valid UTF-8.
pub(super) fn read_utf8(path: &Utf8Path) -> Result<String, Error> {
    with_parent_dir(path, keys::STDLIB_PATH_ACTION_READ, |handle, entry| {
        handle.read_to_string(Utf8Path::new(entry))
    })
}

/// Count the lines in the file at `path`.
///
/// # Errors
///
/// Returns a template error when the file cannot be opened or read as UTF-8.
pub(super) fn linecount(path: &Utf8Path) -> Result<usize, Error> {
    let content = read_utf8(path)?;
    Ok(content.lines().count())
}

/// Open the file at `path` for reading through a capability handle.
///
/// # Errors
///
/// Returns a template error when the parent directory cannot be opened or the
/// target file cannot be opened for reading.
pub(crate) fn open_file(path: &Utf8Path) -> Result<File, Error> {
    with_parent_dir(path, keys::STDLIB_PATH_ACTION_OPEN_FILE, |handle, entry| {
        let mut options = OpenOptions::new();
        options.read(true);
        handle.open_with(Utf8Path::new(entry), &options)
    })
}
