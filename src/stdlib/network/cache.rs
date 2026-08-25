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
use crate::hex::to_lower_hex;
use crate::localization::{self, keys};
use std::io::Read;

/// A single cache entry addressed relative to the cache directory.
pub(super) struct CacheEntry<'a> {
    /// The capability-scoped directory the entry lives in.
    dir: &'a Dir,
    /// The entry name, relative to the cache directory.
    name: &'a str,
    /// The entry's file name as a path.
    path: Utf8PathBuf,
}

impl<'a> CacheEntry<'a> {
    /// Build an entry for a name within a cache directory.
    pub(super) fn new(dir: &'a Dir, name: &'a str) -> Self {
        Self {
            dir,
            name,
            path: Utf8PathBuf::from(name),
        }
    }

    /// Return the entry's file path.
    pub(super) fn path(&self) -> &Utf8Path {
        self.path.as_path()
    }

    /// Open the entry for writing, truncating any existing content.
    ///
    /// # Errors
    ///
    /// Returns an error when the cache entry cannot be created, opened, or
    /// truncated for writing.
    pub(super) fn open_writer(&self) -> Result<File, Error> {
        open_cache_writer(self.dir, self.path())
    }

    /// Remove the entry file from the cache directory.
    fn remove_file(&self) -> io::Result<()> {
        self.dir.remove_file(self.path())
    }

    /// Return the entry's name.
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

/// Open the cache directory at `relative` under `root`, creating it as needed.
///
/// The relative path is validated against the cache-boundary rules before any
/// directory is created.
///
/// # Errors
///
/// Returns an error when `relative` violates the cache-boundary rules, or when
/// the directory cannot be created or opened under `root`.
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

/// Read a cached entry, returning `None` when it does not exist.
///
/// Enforces the response size limit on the exact bytes read from the entry.
///
/// # Errors
///
/// Returns an error when the entry cannot be opened, its metadata cannot be
/// read, its contents cannot be read, or its contents exceed `limit` bytes.
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
    // Cap the read itself, not just the fstat check. The cache writer truncates
    // and rewrites the same inode, so the entry can grow between the stat above
    // and this read; reading at most `limit + 1` bytes bounds memory and lets an
    // over-limit entry be detected rather than silently truncated.
    file.by_ref()
        .take(limit.saturating_add(1))
        .read_to_end(&mut buf)
        .map_err(|err| {
            tracing::warn!(key = name, error = %err, "cache read failed");
            Error::new(
                ErrorKind::InvalidOperation,
                localization::message(keys::STDLIB_FETCH_CACHE_READ_FAILED)
                    .with_arg("name", name)
                    .with_arg("details", err.to_string())
                    .to_string(),
            )
        })?;
    if u64::try_from(buf.len()).unwrap_or(u64::MAX) > limit {
        return Err(response_limit_error_from_cache(name, limit));
    }
    Ok(buf)
}

/// Open a cache entry for writing, creating and truncating it.
///
/// # Errors
///
/// Returns an error when the entry cannot be created, opened, or truncated for
/// writing in `dir`.
fn open_cache_writer(dir: &Dir, path: &Utf8Path) -> Result<File, Error> {
    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    dir.open_with(path, &options).map_err(|err| {
        tracing::warn!(entry = %path, error = %err, "failed to open cache writer");
        io_error(keys::STDLIB_FETCH_ACTION_OPEN_CACHE_ENTRY, path, err)
    })
}

/// Hash a URL into a SHA-256 cache entry key.
pub(super) fn cache_key(url: &str) -> String {
    to_lower_hex(&Sha256::digest(url.as_bytes()))
}

/// Internal cache configuration and directory handle.
#[derive(Clone)]
pub(super) struct FetchCache {
    /// The capability-scoped cache root directory.
    root: Arc<Dir>,
    /// The cache subdirectory relative to the root.
    relative: Utf8PathBuf,
}

impl FetchCache {
    /// Build the cache handle from the network configuration.
    pub(super) fn new(config: &NetworkConfig) -> Self {
        Self {
            root: Arc::clone(&config.cache_root),
            relative: config.cache_relative.clone(),
        }
    }

    /// Open the configured cache directory.
    ///
    /// # Errors
    ///
    /// Returns an error when the configured relative path violates the
    /// cache-boundary rules, or when the cache directory cannot be created or
    /// opened.
    #[rustfmt::skip]
    pub(super) fn open_dir(&self) -> Result<Dir, Error> { open_cache_dir(&self.root, &self.relative) }
}
