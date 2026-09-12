//! File-system access for the stdlib filters through capability-scoped
//! cap-std handles: resolving a path's parent directory, metadata queries,
//! and the shared open policy that enforces the file-type and symlink rules.
//!
//! Reading an opened file within its byte budget belongs to `bounded_read`;
//! this module only decides what may be opened.
use std::io;

use camino::{Utf8Path, Utf8PathBuf};
#[cfg(unix)]
use cap_std::fs_utf8::OpenOptionsExt;
use cap_std::{
    ambient_authority, fs,
    fs_utf8::{Dir, File, OpenOptions},
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
/// On Unix the open is non-blocking, so a FIFO or device final component
/// cannot wedge the render worker inside `open` even when the caller opted
/// into following symlinks; blocking mode is restored once the opened object
/// is confirmed to be a regular file. The final path component is opened
/// without following symlinks unless `limits.follow_symlinks` opts in, and the
/// opened object must be a regular file, checked on the opened handle so
/// devices and FIFOs are rejected race-free.
///
/// # Errors
///
/// Returns a template error when the parent directory cannot be opened, the
/// target cannot be opened, the final component is a symlink while following
/// is disabled, the opened object is not a regular file, or blocking mode
/// cannot be restored.
pub(crate) fn open_file_checked(path: &Utf8Path, limits: &FileReadLimits) -> Result<File, Error> {
    let parent = open_parent_dir(path)?;
    let mut options = OpenOptions::new();
    options.read(true);
    // `O_NONBLOCK` is applied unconditionally so a FIFO or device cannot block
    // the open under either symlink policy; `O_NOFOLLOW` is added only by the
    // default policy, which rejects a symlink final component.
    #[cfg(unix)]
    apply_unix_open_flags(&mut options, limits.follow_symlinks, path)?;
    #[cfg(windows)]
    if !limits.follow_symlinks {
        reject_windows_symlink(&parent, path)?;
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
    #[cfg(unix)]
    restore_blocking(&file, path)?;
    Ok(file)
}

/// Set `O_NONBLOCK`, and `O_NOFOLLOW` unless the policy follows symlinks.
///
/// `O_NONBLOCK` is unconditional: a FIFO or device must not block the open,
/// including on the opt-in path that follows a symlink to one. `O_NOFOLLOW` is
/// added only by the default policy, which rejects a symlink final component.
///
/// # Errors
///
/// Returns a template error when the platform flag bits do not fit an `i32`.
#[cfg(unix)]
fn apply_unix_open_flags(
    options: &mut OpenOptions,
    follow_symlinks: bool,
    path: &Utf8Path,
) -> Result<(), Error> {
    let mut flags = OFlags::NONBLOCK;
    if !follow_symlinks {
        flags |= OFlags::NOFOLLOW;
    }
    let bits = i32::try_from(flags.bits()).map_err(|err| {
        io_to_error(
            path,
            &localization::message(keys::STDLIB_PATH_ACTION_OPEN_FILE),
            io::Error::new(io::ErrorKind::InvalidInput, err),
        )
    })?;
    options.custom_flags(bits);
    Ok(())
}

/// Reject a symlink final component ahead of an open on Windows.
///
/// Windows exposes no `O_NOFOLLOW` through cap-std, so the pre-open
/// `symlink_metadata` check is the platform's best available guard.
///
/// # Errors
///
/// Returns a template error when the metadata cannot be read or names a
/// symlink.
#[cfg(windows)]
fn reject_windows_symlink(parent: &ParentDir, path: &Utf8Path) -> Result<(), Error> {
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
    Ok(())
}

/// Clear `O_NONBLOCK` from `file` after a non-blocking policy open.
///
/// # Errors
///
/// Returns a template error when the flag swap fails; the caller treats this
/// as an unreadable file rather than continuing with non-blocking semantics.
#[cfg(unix)]
fn restore_blocking(file: &File, path: &Utf8Path) -> Result<(), Error> {
    let fd = std::os::fd::AsFd::as_fd(file);
    let flags = rustix::fs::fcntl_getfl(fd).map_err(|err| {
        io_to_error(
            path,
            &localization::message(keys::STDLIB_PATH_ACTION_OPEN_FILE),
            io::Error::from(err),
        )
    })?;
    rustix::fs::fcntl_setfl(fd, flags & !OFlags::NONBLOCK).map_err(|err| {
        io_to_error(
            path,
            &localization::message(keys::STDLIB_PATH_ACTION_OPEN_FILE),
            io::Error::from(err),
        )
    })
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
