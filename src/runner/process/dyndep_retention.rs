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

/// Relative path of the advisory lock file guarding sidecar publication.
const DYNDEP_LOCK: &str = ".netsuke/dyndep/.netsuke-publication.lock";

/// Exclusive sidecar-directory lease held while a serial command consumes a bundle.
pub(crate) struct DyndepPublicationLease {
    /// Held lock file, absent for a sidecar-free bundle.
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
    telemetry::instrument_retention(
        || prune_dyndep_cache_inner(dir, current),
        |summary| (summary.reclaimed_files, summary.reclaimed_bytes),
    )
}

/// Prune an unleased cache after checking its directory and acquiring its lease.
fn prune_dyndep_cache_inner(dir: &Dir, current: &[GeneratedDyndep]) -> Result<RetentionSummary> {
    if !dyndep_directory_exists(dir)? {
        return Ok(RetentionSummary::default());
    }
    let lease = DyndepPublicationLease::acquire(dir)?;
    prune_dyndep_sidecars_inner(dir, &lease, current, RetentionPolicy::standard())
}

/// Fixed, deterministic budget for obsolete content-addressed sidecars.
#[derive(Clone, Copy)]
pub(super) struct RetentionPolicy {
    /// Maximum number of obsolete sidecars kept after one publication.
    max_files: usize,
    /// Maximum bytes of obsolete sidecars kept after one publication.
    max_bytes: u64,
}

impl RetentionPolicy {
    /// Return the repository-standard retention budget.
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
    /// Number of obsolete sidecar files removed by the retention pass.
    reclaimed_files: u64,
    /// Number of bytes reclaimed across the removed sidecars.
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

/// Prune obsolete sidecars under an active lease, if the lease holds a lock.
///
/// # Errors
///
/// Returns an error when the sidecar directory cannot be traversed or a
/// candidate cannot be inspected or removed.
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
    let mut pass = RetentionPass {
        current_paths,
        retained: RetentionSelection::new(policy),
        summary,
    };
    for entry_result in dir
        .read_dir(DYNDEP_DIR)
        .with_context(|| retention_error(Utf8Path::new(DYNDEP_DIR)))?
    {
        retain_directory_entry(dir, entry_result, &mut pass)?;
    }
    Ok(())
}

/// Mutable state scoped to one leased directory traversal.
struct RetentionPass<'current, 'summary> {
    /// Relative paths of the sidecars the current command still consumes.
    current_paths: &'current HashSet<&'current str>,
    /// Lexical selection state for obsolete sidecar retention.
    retained: RetentionSelection,
    /// Aggregate counts of removed candidates so far.
    summary: &'summary mut RetentionSummary,
}

/// Apply retention policy to one directory entry while the lease is held.
fn retain_directory_entry(
    dir: &Dir,
    entry_result: std::io::Result<cap_std::fs_utf8::DirEntry>,
    pass: &mut RetentionPass<'_, '_>,
) -> Result<()> {
    let entry = entry_result.with_context(|| retention_error(Utf8Path::new(DYNDEP_DIR)))?;
    let name = entry
        .file_name()
        .with_context(|| retention_error(Utf8Path::new(DYNDEP_DIR)))?;
    let path = Utf8Path::new(DYNDEP_DIR).join(name);
    if path.as_str() == DYNDEP_LOCK || pass.current_paths.contains(path.as_str()) {
        return Ok(());
    }
    if has_extension(&path, "tmp") {
        // Every current atomic writer owns the same exclusive lease, so a
        // temporary file observed here cannot belong to an active Netsuke
        // publication protocol.
        let bytes = candidate_size(dir, &path)?;
        return remove_candidate(dir, &path, bytes, pass.summary);
    }
    if is_obsolete_sidecar(&path, pass.current_paths) {
        retain_or_remove_sidecar(dir, path, &mut pass.retained, pass.summary)?;
    }
    Ok(())
}

/// Bounded lexical selection state used only while traversing one retention directory.
struct RetentionSelection {
    /// Budget constraining which obsolete sidecars survive selection.
    policy: RetentionPolicy,
    /// Retained `${path} -> bytes` pairs under lexical order.
    paths: BTreeMap<Utf8PathBuf, u64>,
    /// Bytes consumed by the currently retained pairs.
    retained_bytes: u64,
}

impl RetentionSelection {
    /// Construct empty selection state from a retention policy.
    const fn new(policy: RetentionPolicy) -> Self {
        Self {
            policy,
            paths: BTreeMap::new(),
            retained_bytes: 0,
        }
    }

    /// Reapply lexical greedy selection to the retained paths and one candidate.
    ///
    /// A directory does not promise an entry order. Reconsidering the at-most
    /// `max_files + 1` paths therefore lets an earlier path evict every later
    /// path it displaces without retaining an unbounded candidate list.
    fn select(&mut self, path: Utf8PathBuf, bytes: u64) -> Vec<(Utf8PathBuf, u64)> {
        self.paths.insert(path, bytes);
        let candidates = std::mem::take(&mut self.paths);
        self.retained_bytes = 0;
        let mut reclaimed = Vec::with_capacity(candidates.len());

        for (candidate_path, candidate_bytes) in candidates {
            let has_file_capacity = self.paths.len() < self.policy.max_files;
            let has_byte_capacity =
                candidate_bytes <= self.policy.max_bytes.saturating_sub(self.retained_bytes);
            if has_file_capacity && has_byte_capacity {
                self.retained_bytes = self.retained_bytes.saturating_add(candidate_bytes);
                self.paths.insert(candidate_path, candidate_bytes);
            } else {
                reclaimed.push((candidate_path, candidate_bytes));
            }
        }
        reclaimed
    }
}

/// Retain an obsolete sidecar or reclaim paths rejected by lexical selection.
fn retain_or_remove_sidecar(
    dir: &Dir,
    path: Utf8PathBuf,
    retained: &mut RetentionSelection,
    summary: &mut RetentionSummary,
) -> Result<()> {
    let bytes = candidate_size(dir, &path)?;
    for (reclaimed_path, reclaimed_bytes) in retained.select(path, bytes) {
        remove_candidate(dir, &reclaimed_path, reclaimed_bytes, summary)?;
    }
    Ok(())
}

/// Return whether `path` is an obsolete `.dd` sidecar not in current use.
fn is_obsolete_sidecar(path: &Utf8Path, current_paths: &HashSet<&str>) -> bool {
    has_extension(path, "dd") && !current_paths.contains(path.as_str())
}

/// Return whether `path`'s extension matches `extension` case-insensitively.
fn has_extension(path: &Utf8Path, extension: &str) -> bool {
    path.extension()
        .is_some_and(|actual| actual.eq_ignore_ascii_case(extension))
}

/// Return a candidate's byte length from its metadata.
///
/// # Errors
///
/// Returns an error when the candidate's metadata cannot be read.
fn candidate_size(dir: &Dir, path: &Utf8Path) -> Result<u64> {
    dir.metadata(path)
        .map(|metadata| metadata.len())
        .with_context(|| retention_error(path))
}

/// Remove a reclaimed sidecar and accumulate its size into the summary.
///
/// # Errors
///
/// Returns an error when the sidecar cannot be removed.
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

/// Return whether the dyndep sidecar directory exists under `dir`.
///
/// # Errors
///
/// Returns an error when the directory cannot be opened for a reason other
/// than its absence.
fn dyndep_directory_exists(dir: &Dir) -> Result<bool> {
    match dir.open_dir(DYNDEP_DIR) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).with_context(|| retention_error(Utf8Path::new(DYNDEP_DIR))),
    }
}

/// Build the localized retention error message for a dyndep `path`.
fn retention_error(path: &Utf8Path) -> crate::localization::LocalizedMessage {
    localization::message(keys::RUNNER_IO_DYNDEP_RETENTION).with_arg("path", path.as_str())
}

#[cfg(test)]
#[path = "dyndep_retention_tests.rs"]
mod retention_tests;
