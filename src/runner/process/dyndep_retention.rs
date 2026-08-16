//! Coordinated, bounded retention for generated dyndep sidecars.
//!
//! A content-addressed sidecar is immutable once published, but changed
//! manifests otherwise accumulate historical `.dd` files indefinitely. This
//! module owns a capability-scoped advisory lease and deterministic cleanup.
//! Every serial command holds the lease until it finishes consuming its bundle,
//! so a concurrent Netsuke command cannot prune active sidecars.

use super::{DYNDEP_DIR, dyndep_telemetry as telemetry};
use crate::localization::{self, keys};
use crate::ninja_gen::GeneratedDyndep;
use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::fs_utf8::{Dir, File, OpenOptions};
use fs4::FileExt;
use std::{
    collections::{BTreeMap, HashSet},
    io::ErrorKind,
};

/// Maximum number of obsolete sidecars retained after one publication.
pub const MAX_RETAINED_DYNDEP_FILES: usize = 32;
/// Maximum bytes occupied by obsolete sidecars retained after one publication.
pub(super) const MAX_RETAINED_DYNDEP_BYTES: u64 = 1024 * 1024;

const DYNDEP_LOCK: &str = ".netsuke/dyndep/.netsuke-publication.lock";

/// Exclusive sidecar-directory lease held while a serial command consumes a bundle.
pub(crate) struct DyndepPublicationLease {
    lock_file: Option<File>,
}

impl DyndepPublicationLease {
    /// Construct a lease for a sidecar-free bundle without opening a directory.
    pub(crate) const fn empty() -> Self {
        Self { lock_file: None }
    }

    /// Acquire the advisory sidecar-directory lease through `dir`.
    pub(crate) fn acquire(dir: &Dir) -> Result<Self> {
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true);
        let file = dir
            .open_with(DYNDEP_LOCK, &options)
            .with_context(|| retention_error(Utf8Path::new(DYNDEP_LOCK)))?;
        let std_file = file.into_std();
        match FileExt::try_lock(&std_file) {
            Ok(()) => {}
            Err(fs4::TryLockError::WouldBlock) => {
                tracing::debug!(
                    lock_path = DYNDEP_LOCK,
                    "waiting for dyndep publication lease"
                );
                FileExt::lock(&std_file)
                    .with_context(|| retention_error(Utf8Path::new(DYNDEP_LOCK)))?;
            }
            Err(fs4::TryLockError::Error(error)) => {
                return Err(error).with_context(|| retention_error(Utf8Path::new(DYNDEP_LOCK)));
            }
        }
        Ok(Self {
            lock_file: Some(File::from_std(std_file)),
        })
    }

    /// Apply the standard retention policy while this lease remains alive.
    pub(crate) fn prune(&self, dir: &Dir, current: &[GeneratedDyndep]) -> Result<RetentionSummary> {
        prune_dyndep_sidecars(dir, self, current, RetentionPolicy::standard())
    }
}

/// Apply retention after a sidecar-free clean command succeeds.
pub(crate) fn prune_dyndep_cache(
    dir: &Dir,
    current: &[GeneratedDyndep],
) -> Result<RetentionSummary> {
    if !dyndep_directory_exists(dir)? {
        return Ok(RetentionSummary::default());
    }
    let lease = DyndepPublicationLease::acquire(dir)?;
    lease.prune(dir, current)
}

/// Fixed, deterministic budget for obsolete content-addressed sidecars.
#[derive(Clone, Copy)]
pub(super) struct RetentionPolicy {
    max_files: usize,
    max_bytes: u64,
}

impl RetentionPolicy {
    const fn standard() -> Self {
        Self {
            max_files: MAX_RETAINED_DYNDEP_FILES,
            max_bytes: MAX_RETAINED_DYNDEP_BYTES,
        }
    }

    #[cfg(test)]
    pub(super) const fn new(max_files: usize, max_bytes: u64) -> Self {
        Self {
            max_files,
            max_bytes,
        }
    }
}

/// Aggregate, non-sensitive result of one retention pass.
#[derive(Default)]
pub(crate) struct RetentionSummary {
    reclaimed_files: u64,
    reclaimed_bytes: u64,
}

/// Remove obsolete sidecars through an already-held publication lease.
pub(super) fn prune_dyndep_sidecars(
    dir: &Dir,
    lease: &DyndepPublicationLease,
    current: &[GeneratedDyndep],
    policy: RetentionPolicy,
) -> Result<RetentionSummary> {
    telemetry::instrument_retention(
        || prune_dyndep_sidecars_inner(dir, lease, current, policy),
        |summary| (summary.reclaimed_files, summary.reclaimed_bytes),
    )
}

fn prune_dyndep_sidecars_inner(
    dir: &Dir,
    lease: &DyndepPublicationLease,
    current: &[GeneratedDyndep],
    policy: RetentionPolicy,
) -> Result<RetentionSummary> {
    if lease.lock_file.is_none() {
        return Ok(RetentionSummary::default());
    }
    let current_paths = current
        .iter()
        .map(|sidecar| sidecar.relative_path().as_str())
        .collect::<HashSet<_>>();
    let mut summary = RetentionSummary::default();
    retain_obsolete_sidecars(dir, &current_paths, policy, &mut summary)?;
    Ok(summary)
}

/// Select obsolete sidecars during one directory traversal with bounded memory.
fn retain_obsolete_sidecars(
    dir: &Dir,
    current_paths: &HashSet<&str>,
    policy: RetentionPolicy,
    summary: &mut RetentionSummary,
) -> Result<()> {
    let mut retained = RetentionSelection::new(policy);
    for entry_result in dir
        .read_dir(DYNDEP_DIR)
        .with_context(|| retention_error(Utf8Path::new(DYNDEP_DIR)))?
    {
        let entry = entry_result.with_context(|| retention_error(Utf8Path::new(DYNDEP_DIR)))?;
        let name = entry
            .file_name()
            .with_context(|| retention_error(Utf8Path::new(DYNDEP_DIR)))?;
        let path = Utf8Path::new(DYNDEP_DIR).join(name);
        if path.as_str() == DYNDEP_LOCK || current_paths.contains(path.as_str()) {
            continue;
        }
        if has_extension(&path, "tmp") {
            // Every current atomic writer owns the same exclusive lease, so a
            // temporary file observed here cannot belong to an active Netsuke
            // publication protocol.
            let bytes = candidate_size(dir, &path)?;
            remove_candidate(dir, &path, bytes, summary)?;
        } else if is_obsolete_sidecar(&path, current_paths) {
            retain_or_remove_sidecar(dir, path, &mut retained, summary)?;
        }
    }
    Ok(())
}

/// Bounded selection state used only while traversing one retention directory.
struct RetentionSelection {
    policy: RetentionPolicy,
    paths: BTreeMap<Utf8PathBuf, u64>,
    retained_bytes: u64,
}

impl RetentionSelection {
    const fn new(policy: RetentionPolicy) -> Self {
        Self {
            policy,
            paths: BTreeMap::new(),
            retained_bytes: 0,
        }
    }

    const fn can_retain(&self, bytes: u64) -> bool {
        bytes <= self.policy.max_bytes.saturating_sub(self.retained_bytes)
    }

    fn is_full(&self) -> bool {
        self.paths.len() == self.policy.max_files
    }

    fn largest_path(&self) -> Option<&Utf8PathBuf> {
        self.paths.last_key_value().map(|(path, _)| path)
    }

    fn insert(&mut self, path: Utf8PathBuf, bytes: u64) {
        self.retained_bytes = self.retained_bytes.saturating_add(bytes);
        self.paths.insert(path, bytes);
    }

    fn pop_largest(&mut self) -> Option<(Utf8PathBuf, u64)> {
        let candidate = self.paths.pop_last()?;
        self.retained_bytes = self.retained_bytes.saturating_sub(candidate.1);
        Some(candidate)
    }
}

/// Retain an obsolete sidecar or reclaim it immediately when it cannot fit.
fn retain_or_remove_sidecar(
    dir: &Dir,
    path: Utf8PathBuf,
    retained: &mut RetentionSelection,
    summary: &mut RetentionSummary,
) -> Result<()> {
    let bytes = candidate_size(dir, &path)?;
    if !retained.is_full() && retained.can_retain(bytes) {
        retained.insert(path, bytes);
        return Ok(());
    }
    let Some(largest_path) = retained.largest_path() else {
        remove_candidate(dir, &path, bytes, summary)?;
        return Ok(());
    };
    let replaces_largest = path < *largest_path;
    let bytes_after_replacement = retained
        .retained_bytes
        .saturating_sub(
            retained
                .paths
                .get(largest_path)
                .copied()
                .unwrap_or_default(),
        )
        .saturating_add(bytes);
    if !replaces_largest || bytes_after_replacement > retained.policy.max_bytes {
        remove_candidate(dir, &path, bytes, summary)?;
        return Ok(());
    }

    let Some((evicted_path, evicted_bytes)) = retained.pop_largest() else {
        remove_candidate(dir, &path, bytes, summary)?;
        return Ok(());
    };
    remove_candidate(dir, &evicted_path, evicted_bytes, summary)?;
    retained.insert(path, bytes);
    Ok(())
}

fn is_obsolete_sidecar(path: &Utf8Path, current_paths: &HashSet<&str>) -> bool {
    has_extension(path, "dd") && !current_paths.contains(path.as_str())
}

fn has_extension(path: &Utf8Path, extension: &str) -> bool {
    path.extension()
        .is_some_and(|actual| actual.eq_ignore_ascii_case(extension))
}

fn candidate_size(dir: &Dir, path: &Utf8Path) -> Result<u64> {
    dir.metadata(path)
        .map(|metadata| metadata.len())
        .with_context(|| retention_error(path))
}

fn remove_candidate(
    dir: &Dir,
    path: &Utf8Path,
    bytes: u64,
    summary: &mut RetentionSummary,
) -> Result<()> {
    dir.remove_file(path)
        .with_context(|| retention_error(path))?;
    summary.reclaimed_files += 1;
    summary.reclaimed_bytes = summary.reclaimed_bytes.saturating_add(bytes);
    Ok(())
}

fn dyndep_directory_exists(dir: &Dir) -> Result<bool> {
    match dir.open_dir(DYNDEP_DIR) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).with_context(|| retention_error(Utf8Path::new(DYNDEP_DIR))),
    }
}

fn retention_error(path: &Utf8Path) -> crate::localization::LocalizedMessage {
    localization::message(keys::RUNNER_IO_DYNDEP_RETENTION).with_arg("path", path.as_str())
}

#[cfg(test)]
#[path = "dyndep_retention_tests.rs"]
mod retention_tests;
