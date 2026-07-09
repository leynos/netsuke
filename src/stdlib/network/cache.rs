//! Fetch cache storage for the stdlib network helpers.
//!
//! Cache entries live under a capability-scoped directory derived from the
//! workspace root. Keys are SHA-256 digests of the fetched URL, so lookups
//! never trust remote input as a path. Consumed by the `fetch` helper in the
//! parent module.

use std::{io, sync::Arc};

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::fs_utf8::{Dir, File, OpenOptions};
use minijinja::{Error, ErrorKind};
use sha2::{Digest, Sha256};

use super::{NetworkConfig, StdlibConfig, io_error, response_limit_error_from_cache};
use crate::localization::{self, keys};
use std::io::Read;

/// A single cache entry addressed relative to the cache directory.
pub(super) struct CacheEntry<'a> {
    dir: &'a Dir,
    name: &'a str,
    path: Utf8PathBuf,
}

impl<'a> CacheEntry<'a> {
    pub(super) fn new(dir: &'a Dir, name: &'a str) -> Self {
        Self {
            dir,
            name,
            path: Utf8PathBuf::from(name),
        }
    }

    pub(super) fn path(&self) -> &Utf8Path {
        self.path.as_path()
    }

    pub(super) fn open_writer(&self) -> Result<File, Error> {
        open_cache_writer(self.dir, self.path())
    }

    fn remove_file(&self) -> io::Result<()> {
        self.dir.remove_file(self.path())
    }

    pub(super) const fn name(&self) -> &str {
        self.name
    }
}

/// Remove a partially written cache entry, logging (rather than propagating)
/// any cleanup failure so the original fetch error stays primary.
pub(super) fn discard_partial_cache(cache: &CacheEntry<'_>) {
    if let Err(remove_err) = cache.remove_file()
        && remove_err.kind() != io::ErrorKind::NotFound
    {
        tracing::warn!(
            "failed to clean up partial fetch cache '{}': {remove_err}",
            cache.name()
        );
    }
}

pub(super) fn open_cache_dir(root: &Dir, relative: &Utf8Path) -> Result<Dir, Error> {
    tracing::debug!(cache_dir = %relative, "opening fetch cache directory");
    if let Err(err) = StdlibConfig::validate_cache_relative(relative) {
        return Err(Error::new(ErrorKind::InvalidOperation, err.to_string()));
    }

    root.create_dir_all(relative).map_err(|err| {
        tracing::warn!(cache_dir = %relative, error = %err, "failed to create fetch cache directory");
        io_error(keys::STDLIB_FETCH_ACTION_CREATE_CACHE_DIR, relative, err)
    })?;
    root.open_dir(relative).map_err(|err| {
        tracing::warn!(cache_dir = %relative, error = %err, "failed to open fetch cache directory");
        io_error(keys::STDLIB_FETCH_ACTION_OPEN_CACHE_DIR, relative, err)
    })
}

pub(super) fn read_cached(dir: &Dir, name: &str, limit: u64) -> Result<Option<Vec<u8>>, Error> {
    let path = Utf8Path::new(name);
    let mut options = OpenOptions::new();
    options.read(true);
    match dir.open_with(path, &options) {
        Ok(file) => read_cached_file(name, file, limit).map(Some),
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            tracing::debug!(key = name, "cache entry not found");
            Ok(None)
        }
        Err(err) => {
            tracing::warn!(key = name, error = %err, "cache read failed");
            Err(Error::new(
                ErrorKind::InvalidOperation,
                localization::message(keys::STDLIB_FETCH_CACHE_OPEN_FAILED)
                    .with_arg("name", name)
                    .with_arg("details", err.to_string())
                    .to_string(),
            ))
        }
    }
}

/// Read an opened cache entry, enforcing the response size limit.
///
/// The size check reads metadata from the open file handle (an `fstat` on the
/// already-open descriptor) rather than re-resolving `name` through the
/// directory, so the limit applies to the exact bytes subsequently consumed and
/// cannot be bypassed by another process replacing the entry between the stat
/// and the read.
fn read_cached_file(name: &str, mut file: File, limit: u64) -> Result<Vec<u8>, Error> {
    let metadata = file.metadata().map_err(|err| {
        tracing::warn!(key = name, error = %err, "cache read failed");
        io_error(
            keys::STDLIB_FETCH_ACTION_STAT_CACHE,
            Utf8Path::new(name),
            err,
        )
    })?;
    if metadata.len() > limit {
        return Err(response_limit_error_from_cache(name, limit));
    }
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).map_err(|err| {
        tracing::warn!(key = name, error = %err, "cache read failed");
        Error::new(
            ErrorKind::InvalidOperation,
            localization::message(keys::STDLIB_FETCH_CACHE_READ_FAILED)
                .with_arg("name", name)
                .with_arg("details", err.to_string())
                .to_string(),
        )
    })?;
    Ok(buf)
}

fn open_cache_writer(dir: &Dir, path: &Utf8Path) -> Result<File, Error> {
    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    dir.open_with(path, &options).map_err(|err| {
        tracing::warn!(entry = %path, error = %err, "failed to open cache writer");
        io_error(keys::STDLIB_FETCH_ACTION_OPEN_CACHE_ENTRY, path, err)
    })
}

pub(super) fn cache_key(url: &str) -> String {
    let digest = Sha256::digest(url.as_bytes());
    hex_string(&digest)
}

pub(super) fn hex_string(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        // `from_digit` cannot fail for values below 16; fall back to '0'
        // rather than panicking to keep this helper total.
        out.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('0'));
        out.push(char::from_digit(u32::from(byte & 0x0f), 16).unwrap_or('0'));
    }
    out
}

/// Internal cache configuration and directory handle.
#[derive(Clone)]
pub(super) struct FetchCache {
    root: Arc<Dir>,
    relative: Utf8PathBuf,
}

impl FetchCache {
    pub(super) fn new(config: &NetworkConfig) -> Self {
        Self {
            root: Arc::clone(&config.cache_root),
            relative: config.cache_relative.clone(),
        }
    }

    #[rustfmt::skip]
    pub(super) fn open_dir(&self) -> Result<Dir, Error> { open_cache_dir(&self.root, &self.relative) }
}
