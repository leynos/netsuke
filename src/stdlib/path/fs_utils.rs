use std::io;

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{
    ambient_authority, fs,
    fs_utf8::{Dir, File, OpenOptions},
};
use minijinja::Error;

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
    parent_dir(path).map_err(|err| io_to_error(path, "open directory", err))
}

pub(crate) fn file_type_matches<F>(path: &Utf8Path, predicate: F) -> Result<bool, Error>
where
    F: Fn(fs::FileType) -> bool,
{
    match parent_dir(path) {
        Ok(parent) => match parent.handle.symlink_metadata(Utf8Path::new(&parent.entry)) {
            Ok(metadata) => Ok(predicate(metadata.file_type())),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(err) => Err(io_to_error(path, "stat", err)),
        },
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(io_to_error(path, "open directory", err)),
    }
}

pub(super) fn file_size(path: &Utf8Path) -> Result<u64, Error> {
    let parent = open_parent_dir(path)?;
    parent
        .handle
        .metadata(Utf8Path::new(&parent.entry))
        .map(|metadata| metadata.len())
        .map_err(|err| io_to_error(path, "stat", err))
}

pub(super) fn read_utf8(path: &Utf8Path) -> Result<String, Error> {
    let parent = open_parent_dir(path)?;
    parent
        .handle
        .read_to_string(Utf8Path::new(&parent.entry))
        .map_err(|err| io_to_error(path, "read", err))
}

pub(super) fn linecount(path: &Utf8Path) -> Result<usize, Error> {
    let content = read_utf8(path)?;
    Ok(content.lines().count())
}

pub(crate) fn open_file(path: &Utf8Path) -> Result<File, Error> {
    let parent = open_parent_dir(path)?;
    let mut options = OpenOptions::new();
    options.read(true);
    parent
        .handle
        .open_with(Utf8Path::new(&parent.entry), &options)
        .map_err(|err| io_to_error(path, "open file", err))
}
