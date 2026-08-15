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
use std::io::ErrorKind;

/// Maximum number of obsolete sidecars retained after one publication.
pub(super) const MAX_RETAINED_DYNDEP_FILES: usize = 32;
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
        FileExt::lock(&std_file).with_context(|| retention_error(Utf8Path::new(DYNDEP_LOCK)))?;
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
        .collect::<std::collections::HashSet<_>>();
    let mut summary = RetentionSummary::default();
    remove_stale_temporary_files(dir, &current_paths, &mut summary)?;
    let mut retained_paths = Vec::with_capacity(policy.max_files);
    let mut retained_bytes = 0_u64;
    let mut cursor = None;

    while retained_paths.len() < policy.max_files {
        let scan = RetentionScan {
            current_paths: &current_paths,
            cursor: cursor.as_deref(),
            remaining_bytes: policy.max_bytes.saturating_sub(retained_bytes),
        };
        let Some((path, bytes)) = next_obsolete_sidecar_after(dir, &scan, &mut summary)? else {
            break;
        };
        cursor = Some(path.clone());
        retained_bytes = retained_bytes.saturating_add(bytes);
        retained_paths.push(path);
    }

    remove_unretained_sidecars(dir, &current_paths, &retained_paths, &mut summary)?;
    Ok(summary)
}

/// Remove interrupted atomic-write files after acquiring the publication lease.
fn remove_stale_temporary_files(
    dir: &Dir,
    current_paths: &std::collections::HashSet<&str>,
    summary: &mut RetentionSummary,
) -> Result<()> {
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
        }
    }
    Ok(())
}

/// Inputs for one bounded, lexicographic sidecar-selection pass.
struct RetentionScan<'paths, 'cursor> {
    current_paths: &'paths std::collections::HashSet<&'paths str>,
    cursor: Option<&'cursor Utf8Path>,
    remaining_bytes: u64,
}

/// Find the next obsolete sidecar without retaining the directory's full contents.
fn next_obsolete_sidecar_after(
    dir: &Dir,
    scan: &RetentionScan<'_, '_>,
    summary: &mut RetentionSummary,
) -> Result<Option<(Utf8PathBuf, u64)>> {
    let mut next: Option<(Utf8PathBuf, u64)> = None;
    for entry_result in dir
        .read_dir(DYNDEP_DIR)
        .with_context(|| retention_error(Utf8Path::new(DYNDEP_DIR)))?
    {
        let entry = entry_result.with_context(|| retention_error(Utf8Path::new(DYNDEP_DIR)))?;
        let name = entry
            .file_name()
            .with_context(|| retention_error(Utf8Path::new(DYNDEP_DIR)))?;
        let path = Utf8Path::new(DYNDEP_DIR).join(name);
        if !is_obsolete_sidecar(&path, scan.current_paths) {
            continue;
        }
        if scan
            .cursor
            .is_some_and(|after| path.as_str() <= after.as_str())
        {
            continue;
        }
        let bytes = candidate_size(dir, &path)?;
        if bytes > scan.remaining_bytes {
            // Retention only reduces the remaining byte budget, so this
            // candidate can never become eligible in a later pass.
            remove_candidate(dir, &path, bytes, summary)?;
            continue;
        }
        if next
            .as_ref()
            .is_none_or(|(candidate, _)| path.as_str() < candidate.as_str())
        {
            next = Some((path, bytes));
        }
    }
    Ok(next)
}

/// Remove every obsolete sidecar that was not selected by the bounded policy.
fn remove_unretained_sidecars(
    dir: &Dir,
    current_paths: &std::collections::HashSet<&str>,
    retained: &[Utf8PathBuf],
    summary: &mut RetentionSummary,
) -> Result<()> {
    for entry_result in dir
        .read_dir(DYNDEP_DIR)
        .with_context(|| retention_error(Utf8Path::new(DYNDEP_DIR)))?
    {
        let entry = entry_result.with_context(|| retention_error(Utf8Path::new(DYNDEP_DIR)))?;
        let name = entry
            .file_name()
            .with_context(|| retention_error(Utf8Path::new(DYNDEP_DIR)))?;
        let path = Utf8Path::new(DYNDEP_DIR).join(name);
        if is_obsolete_sidecar(&path, current_paths)
            && !retained.iter().any(|retained_path| retained_path == &path)
        {
            let bytes = candidate_size(dir, &path)?;
            remove_candidate(dir, &path, bytes, summary)?;
        }
    }
    Ok(())
}

fn is_obsolete_sidecar(path: &Utf8Path, current_paths: &std::collections::HashSet<&str>) -> bool {
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
