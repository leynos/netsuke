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

use super::io_helpers::io_to_error;
use super::path_utils::normalise_parent;

pub(super) struct ParentDir {
    pub handle: Dir,
    pub entry: String,
    pub dir_path: Utf8PathBuf,
}

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

pub(super) fn file_size(path: &Utf8Path) -> Result<u64, Error> {
    with_parent_dir(path, keys::STDLIB_PATH_ACTION_STAT, |handle, entry| {
        handle
            .metadata(Utf8Path::new(entry))
            .map(|metadata| metadata.len())
    })
}

pub(super) fn read_utf8(path: &Utf8Path) -> Result<String, Error> {
    with_parent_dir(path, keys::STDLIB_PATH_ACTION_READ, |handle, entry| {
        handle.read_to_string(Utf8Path::new(entry))
    })
}

pub(super) fn linecount(path: &Utf8Path) -> Result<usize, Error> {
    let content = read_utf8(path)?;
    Ok(content.lines().count())
}

pub(crate) fn open_file(path: &Utf8Path) -> Result<File, Error> {
    with_parent_dir(path, keys::STDLIB_PATH_ACTION_OPEN_FILE, |handle, entry| {
        let mut options = OpenOptions::new();
        options.read(true);
        handle.open_with(Utf8Path::new(entry), &options)
    })
}
