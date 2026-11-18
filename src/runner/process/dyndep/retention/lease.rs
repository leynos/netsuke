//! Advisory publication lease for the generated dyndep directory.
//!
//! The lease is an exclusive advisory lock on a file inside the verified
//! dyndep directory. Every serial command holds it from publication until it
//! finishes consuming its bundle, so a concurrent Netsuke command cannot prune
//! sidecars that are still in use. The lock coordinates cooperating Netsuke
//! processes only; it does not defend against a process that ignores it.

use super::super::directory::{DyndepDirectory, entry_display_path};
use super::super::telemetry;
use super::{
    DYNDEP_LOCK, RetentionPolicy, RetentionSummary, prune_verified_sidecars, retention_error,
};
use crate::ninja_gen::GeneratedDyndep;
use anyhow::{Context, Result};
use cap_std::fs_utf8::{Dir, File, OpenOptions};
use fs4::FileExt;

/// Exclusive sidecar-directory lease held while a serial command consumes a bundle.
pub(crate) struct DyndepPublicationLease {
    /// Held lock file, absent for a sidecar-free bundle.
    lock_file: Option<File>,
    /// Verified dyndep directory the lock file belongs to.
    dyndep_dir: Option<Dir>,
}

impl DyndepPublicationLease {
    /// Construct a lease for a sidecar-free bundle without opening a directory.
    pub(crate) const fn empty() -> Self {
        Self {
            lock_file: None,
            dyndep_dir: None,
        }
    }

    /// Return whether this lease holds no publication lock.
    pub(crate) const fn is_empty(&self) -> bool {
        self.lock_file.is_none()
    }

    /// Acquire the advisory sidecar-directory lease through a verified handle.
    ///
    /// # Errors
    ///
    /// Returns an error when the lock file cannot be opened or locked.
    pub(crate) fn acquire(dyndep: &DyndepDirectory) -> Result<Self> {
        let dir = dyndep.dir();
        let lock_display = entry_display_path(DYNDEP_LOCK);
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true);
        let file = dir
            .open_with(DYNDEP_LOCK, &options)
            .with_context(|| retention_error(&lock_display))?;
        let std_file = file.into_std();
        match FileExt::try_lock(&std_file) {
            Ok(()) => {}
            Err(fs4::TryLockError::WouldBlock) => {
                tracing::debug!(
                    lock_path = lock_display.as_str(),
                    "waiting for dyndep publication lease"
                );
                FileExt::lock(&std_file).with_context(|| retention_error(&lock_display))?;
            }
            Err(fs4::TryLockError::Error(error)) => {
                return Err(error).with_context(|| retention_error(&lock_display));
            }
        }
        Ok(Self {
            lock_file: Some(File::from_std(std_file)),
            dyndep_dir: Some(
                dir.try_clone()
                    .with_context(|| retention_error(&lock_display))?,
            ),
        })
    }

    /// Apply the standard retention policy while this lease remains alive.
    ///
    /// # Errors
    ///
    /// Returns an error when the sidecar directory cannot be traversed or a
    /// candidate cannot be inspected or removed.
    pub(crate) fn prune(&self, current: &[GeneratedDyndep]) -> Result<RetentionSummary> {
        telemetry::instrument_retention(
            || self.prune_inner(current, RetentionPolicy::standard()),
            |summary| (summary.reclaimed_files, summary.reclaimed_bytes),
        )
    }

    /// Prune through the verified directory this lease holds.
    fn prune_inner(
        &self,
        current: &[GeneratedDyndep],
        policy: RetentionPolicy,
    ) -> Result<RetentionSummary> {
        self.dyndep_dir.as_ref().map_or_else(
            || Ok(RetentionSummary::default()),
            |dir| prune_verified_sidecars(dir, self, current, policy),
        )
    }
}
