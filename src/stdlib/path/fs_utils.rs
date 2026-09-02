//! UTF-8 file-system helpers for stdlib filters using cap-std Dir handles: metadata queries,
//! opening files for streaming, and safe error translation.
use std::io::{self, BufRead, BufReader, Read};

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{
    ambient_authority, fs,
    fs_utf8::{Dir, File, OpenOptions, OpenOptionsExt},
};
use minijinja::Error;
#[cfg(unix)]
use rustix::fs::OFlags;

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

/// Per-call limits for the file-reading filters.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FileReadLimits {
    /// Maximum number of bytes the read may consume.
    pub max_bytes: u64,
    /// Whether the final path component may be a symlink.
    pub follow_symlinks: bool,
}

/// Read a bounded chunk of `file`, rejecting reads that exceed `max_bytes`.
///
/// Returns the bytes read so far, or `None` when the source has been
/// exhausted. Returns an error once the running total passes `max_bytes`.
pub(crate) fn read_bounded_chunk<'a>(
    file: &mut File,
    buffer: &'a mut [u8],
    total: &mut u64,
    max_bytes: u64,
    path: &Utf8Path,
) -> Result<Option<&'a [u8]>, Error> {
    let read = file.read(buffer).map_err(|err| {
        io_to_error(path, &localization::message(keys::STDLIB_PATH_ACTION_READ), err)
    })?;
    if read == 0 {
        return Ok(None);
    }
    *total = total.saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
    if *total > max_bytes {
        return Err(file_too_large_error(path, max_bytes));
    }
    Ok(Some(&buffer[..read]))
}

/// Build the localized byte-budget diagnostic for `path` and `limit`.
pub(crate) fn file_too_large_error(path: &Utf8Path, limit: u64) -> Error {
    Error::new(
        minijinja::ErrorKind::InvalidOperation,
        localization::message(keys::STDLIB_PATH_FILE_TOO_LARGE)
            .with_arg("path", path.as_str())
            .with_arg("limit", limit)
            .to_string(),
    )
}

/// Build the localized non-regular-file diagnostic for `path`.
pub(crate) fn not_regular_file_error(path: &Utf8Path) -> Error {
    Error::new(
        minijinja::ErrorKind::InvalidOperation,
        localization::message(keys::STDLIB_PATH_NOT_REGULAR_FILE)
            .with_arg("path", path.as_str())
            .to_string(),
    )
}

/// Open `path` for reading under the file-reading safety policy.
///
/// The final path component is opened without following symlinks unless
/// `limits.follow_symlinks` opts in, and the opened object must be a regular
/// file, checked on the opened handle so devices and FIFOs are rejected
/// race-free.
///
/// # Errors
///
/// Returns a template error when the parent directory cannot be opened, the
/// target cannot be opened, the final component is a symlink while following
/// is disabled, or the opened object is not a regular file.
pub(crate) fn open_file_checked(
    path: &Utf8Path,
    limits: &FileReadLimits,
) -> Result<File, Error> {
    let parent = open_parent_dir(path)?;
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    if !limits.follow_symlinks {
        options.custom_flags(i32::try_from(OFlags::NOFOLLOW.bits()).map_err(|err| {
            io_to_error(
                path,
                &localization::message(keys::STDLIB_PATH_ACTION_OPEN_FILE),
                io::Error::new(io::ErrorKind::InvalidInput, err),
            )
        })?);
    }
    #[cfg(windows)]
    if !limits.follow_symlinks {
        // Windows has no O_NOFOLLOW equivalent through cap-std; reject a
        // symlink final component before opening.
        let metadata = parent
            .handle
            .symlink_metadata(Utf8Path::new(&parent.entry))
            .map_err(|err| {
                io_to_error(
                    path,
                    &localization::message(keys::STDLIB_PATH_ACTION_STAT),
                    err,
                )
            })?;
        if metadata.file_type().is_symlink() {
            return Err(not_regular_file_error(path));
        }
    }
    let file = parent
        .handle
        .open_with(Utf8Path::new(&parent.entry), &options)
        .map_err(|err| {
            io_to_error(
                path,
                &localization::message(keys::STDLIB_PATH_ACTION_OPEN_FILE),
                err,
            )
        })?;
    let metadata = file.metadata().map_err(|err| {
        io_to_error(
            path,
            &localization::message(keys::STDLIB_PATH_ACTION_STAT),
            err,
        )
    })?;
    if !metadata.is_file() {
        return Err(not_regular_file_error(path));
    }
    Ok(file)
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
/// file cannot be read, its contents are not valid UTF-8, or the read exceeds
/// the configured byte budget.
pub(crate) fn read_utf8(path: &Utf8Path, limits: &FileReadLimits) -> Result<String, Error> {
    let mut file = open_file_checked(path, limits)?;
    let mut total: u64 = 0;
    let mut buffer = [0_u8; 8192];
    let mut bytes = Vec::new();
    while let Some(chunk) = read_bounded_chunk(&mut file, &mut buffer, &mut total, limits.max_bytes, path)? {
        bytes.extend_from_slice(chunk);
    }
    String::from_utf8(bytes).map_err(|_| {
        Error::new(
            minijinja::ErrorKind::InvalidOperation,
            localization::message(keys::STDLIB_PATH_IO_INVALID_DATA)
                .with_arg("path", path.as_str())
                .to_string(),
        )
    })
}

/// Count the lines in the file at `path`.
///
/// # Errors
///
/// Returns a template error when the file cannot be opened or read, or when
/// the read exceeds the configured byte budget.
pub(crate) fn linecount(path: &Utf8Path, limits: &FileReadLimits) -> Result<usize, Error> {
    let mut file = open_file_checked(path, limits)?;
    let mut reader = BufReader::new(&mut file);
    let mut lines: usize = 0;
    let mut total: u64 = 0;
    let mut buffer = Vec::new();
    loop {
        buffer.clear();
        let read = reader
            .read_until(b'\n', &mut buffer)
            .map_err(|err| {
                io_to_error(
                    path,
                    &localization::message(keys::STDLIB_PATH_ACTION_READ),
                    err,
                )
            })?;
        if read == 0 {
            break;
        }
        total = total.saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
        if total > limits.max_bytes {
            return Err(file_too_large_error(path, limits.max_bytes));
        }
        if !buffer.is_empty() {
            lines += 1;
        }
    }
    Ok(lines)
}
