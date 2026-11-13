//! Coordinated, bounded retention for generated dyndep sidecars.
//!
//! A content-addressed sidecar is immutable once published, but changed
//! manifests otherwise accumulate historical `.dd` files indefinitely. This
//! module owns a capability-scoped advisory lease and deterministic cleanup.
//! Every serial command holds the lease until it finishes consuming its bundle,
//! so a concurrent Netsuke command cannot prune active sidecars.
//!
//! Every path here is a bare entry name relative to a verified
//! [`DyndepDirectory`] handle. The workspace capability is used only to
//! establish that handle, so retention can never resolve a component outside
//! the generated-state directory it verified.

mod lease;

use super::super::DYNDEP_DIR;
use super::directory::{DyndepDirectory, entry_display_path, is_alias};
use super::telemetry;
use crate::localization::{self, keys};
use crate::ninja_gen::GeneratedDyndep;
use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::fs_utf8::Dir;
use std::collections::{BTreeMap, HashSet};

pub(crate) use lease::DyndepPublicationLease;

/// Maximum number of obsolete sidecars retained after one publication.
pub const MAX_RETAINED_DYNDEP_FILES: usize = 32;
/// Maximum bytes occupied by obsolete sidecars retained after one publication.
pub(super) const MAX_RETAINED_DYNDEP_BYTES: u64 = 1024 * 1024;

/// Entry name of the advisory lock file guarding sidecar publication.
const DYNDEP_LOCK: &str = ".netsuke-publication.lock";

/// Apply retention after a sidecar-free clean command succeeds.
pub(crate) fn prune_dyndep_cache(
    workspace: &Dir,
    current: &[GeneratedDyndep],
) -> Result<RetentionSummary> {
    telemetry::instrument_retention(
        || prune_dyndep_cache_inner(workspace, current),
        |summary| (summary.reclaimed_files, summary.reclaimed_bytes),
    )
}

/// Prune an unleased cache after establishing its verified directory.
fn prune_dyndep_cache_inner(
    workspace: &Dir,
    current: &[GeneratedDyndep],
) -> Result<RetentionSummary> {
    let Some(dyndep) = DyndepDirectory::open_existing(workspace)? else {
        return Ok(RetentionSummary::default());
    };
    let lease = DyndepPublicationLease::acquire(&dyndep)?;
    prune_verified_sidecars(dyndep.dir(), &lease, current, RetentionPolicy::standard())
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
///
/// Test-only seam: production paths prune through
/// [`DyndepPublicationLease::prune`], which owns the verified handle. This entry
/// point lets a test inject a smaller budget and exercise the workspace-level
/// re-verification that a sidecar-free command performs.
#[cfg(test)]
pub(super) fn prune_dyndep_sidecars(
    workspace: &Dir,
    lease: &DyndepPublicationLease,
    current: &[GeneratedDyndep],
    policy: RetentionPolicy,
) -> Result<RetentionSummary> {
    telemetry::instrument_retention(
        || prune_dyndep_sidecars_inner(workspace, lease, current, policy),
        |summary| (summary.reclaimed_files, summary.reclaimed_bytes),
    )
}

/// Verify the generated-state directory, then prune through the lease.
///
/// # Errors
///
/// Returns an error when the generated-state directory is aliased or cannot be
/// opened, or when a candidate cannot be inspected or removed.
#[cfg(test)]
fn prune_dyndep_sidecars_inner(
    workspace: &Dir,
    lease: &DyndepPublicationLease,
    current: &[GeneratedDyndep],
    policy: RetentionPolicy,
) -> Result<RetentionSummary> {
    if lease.is_empty() {
        return Ok(RetentionSummary::default());
    }
    let Some(dyndep) = DyndepDirectory::open_existing(workspace)? else {
        return Ok(RetentionSummary::default());
    };
    prune_verified_sidecars(dyndep.dir(), lease, current, policy)
}

/// Prune obsolete sidecars relative to a verified dyndep directory handle.
fn prune_verified_sidecars(
    dyndep: &Dir,
    lease: &DyndepPublicationLease,
    current: &[GeneratedDyndep],
    policy: RetentionPolicy,
) -> Result<RetentionSummary> {
    if lease.is_empty() {
        return Ok(RetentionSummary::default());
    }
    let current_names = current_sidecar_names(current);
    let mut summary = RetentionSummary::default();
    retain_obsolete_sidecars(dyndep, &current_names, policy, &mut summary)?;
    Ok(summary)
}

/// Collect the bare entry names of the sidecars the current bundle consumes.
fn current_sidecar_names(current: &[GeneratedDyndep]) -> HashSet<&str> {
    current
        .iter()
        .filter_map(|sidecar| dyndep_entry_name(sidecar.relative_path()))
        .collect()
}

/// Return `path`'s bare entry name when it names a direct dyndep child.
fn dyndep_entry_name(path: &Utf8Path) -> Option<&str> {
    let name = path.strip_prefix(DYNDEP_DIR).ok()?;
    (name.components().count() == 1).then(|| name.as_str())
}

/// Select obsolete sidecars during one directory traversal with bounded memory.
fn retain_obsolete_sidecars(
    dyndep: &Dir,
    current_names: &HashSet<&str>,
    policy: RetentionPolicy,
    summary: &mut RetentionSummary,
) -> Result<()> {
    let mut pass = RetentionPass {
        current_names,
        retained: RetentionSelection::new(policy),
        summary,
    };
    for entry_result in dyndep
        .entries()
        .with_context(|| retention_error(Utf8Path::new(DYNDEP_DIR)))?
    {
        retain_directory_entry(dyndep, entry_result, &mut pass)?;
    }
    Ok(())
}

/// Mutable state scoped to one leased directory traversal.
struct RetentionPass<'current, 'summary> {
    /// Bare entry names of the sidecars the current command still consumes.
    current_names: &'current HashSet<&'current str>,
    /// Lexical selection state for obsolete sidecar retention.
    retained: RetentionSelection,
    /// Aggregate counts of removed candidates so far.
    summary: &'summary mut RetentionSummary,
}

/// Apply retention policy to one directory entry while the lease is held.
fn retain_directory_entry(
    dyndep: &Dir,
    entry_result: std::io::Result<cap_std::fs_utf8::DirEntry>,
    pass: &mut RetentionPass<'_, '_>,
) -> Result<()> {
    let entry = entry_result.with_context(|| retention_error(Utf8Path::new(DYNDEP_DIR)))?;
    let name = entry
        .file_name()
        .with_context(|| retention_error(Utf8Path::new(DYNDEP_DIR)))?;
    retain_entry(dyndep, &name, pass)
}

/// Apply retention policy to one entry name while the lease is held.
fn retain_entry(dyndep: &Dir, name: &str, pass: &mut RetentionPass<'_, '_>) -> Result<()> {
    if name == DYNDEP_LOCK || pass.current_names.contains(name) {
        return Ok(());
    }
    let display = entry_display_path(name);
    let metadata = dyndep
        .symlink_metadata(name)
        .with_context(|| retention_error(&display))?;
    if is_alias(&metadata) {
        // A link inside the cache is not a cache entry. Removing it would
        // delete the link, and following it would make its target disposable.
        return Ok(());
    }
    if has_extension(Utf8Path::new(name), "tmp") {
        // Every current atomic writer owns the same exclusive lease, so a
        // temporary file observed here cannot belong to an active Netsuke
        // publication protocol.
        return remove_candidate(dyndep, name, metadata.len(), pass.summary);
    }
    if is_sidecar(Utf8Path::new(name)) {
        retain_or_remove_sidecar(dyndep, name, metadata.len(), pass)?;
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
///
/// Takes the size the traversal already read rather than re-statting the entry:
/// each sidecar would otherwise pay a second metadata round trip, and the
/// second read could disagree with the one the alias check approved.
fn retain_or_remove_sidecar(
    dyndep: &Dir,
    name: &str,
    bytes: u64,
    pass: &mut RetentionPass<'_, '_>,
) -> Result<()> {
    for (reclaimed_name, reclaimed_bytes) in pass.retained.select(Utf8PathBuf::from(name), bytes) {
        remove_candidate(
            dyndep,
            reclaimed_name.as_str(),
            reclaimed_bytes,
            pass.summary,
        )?;
    }
    Ok(())
}

/// Return whether `path` names a `.dd` sidecar.
fn is_sidecar(path: &Utf8Path) -> bool {
    has_extension(path, "dd")
}

/// Return whether `path`'s extension matches `extension` case-insensitively.
fn has_extension(path: &Utf8Path, extension: &str) -> bool {
    path.extension()
        .is_some_and(|actual| actual.eq_ignore_ascii_case(extension))
}

/// Remove a reclaimed sidecar and accumulate its size into the summary.
///
/// # Errors
///
/// Returns an error when the sidecar cannot be removed.
fn remove_candidate(
    dyndep: &Dir,
    name: &str,
    bytes: u64,
    summary: &mut RetentionSummary,
) -> Result<()> {
    let display = entry_display_path(name);
    dyndep
        .remove_file(name)
        .with_context(|| retention_error(&display))?;
    summary.reclaimed_files += 1;
    summary.reclaimed_bytes = summary.reclaimed_bytes.saturating_add(bytes);
    Ok(())
}

/// Build the localized retention error message for a dyndep `path`.
fn retention_error(path: &Utf8Path) -> crate::localization::LocalizedMessage {
    localization::message(keys::RUNNER_IO_DYNDEP_RETENTION).with_arg("path", path.as_str())
}

#[cfg(test)]
mod tests;
