//! Runner command boundary for generated dyndep sidecar publication.
//!
//! Bundle generation is a query. This module owns the one command that turns a
//! generated bundle into working-directory state, opening ambient authority at
//! the outer runner boundary and injecting the resulting `Dir` into the
//! materializer. Only `runner` command handlers may call this module.

use super::super::phony_state::Workspace;
use super::super::process::{self, DyndepPublicationLease};
use crate::ninja_gen::{GeneratedDyndep, GeneratedNinja};
use anyhow::Result;
use cap_std::fs_utf8::Dir;

/// Publication lease that protects one serial bundle while a command consumes it.
///
/// The lease owns the verified dyndep directory handle, so a command that
/// published a bundle prunes through exactly the directory it wrote, never
/// through a fresh resolution of the workspace capability.
pub(in crate::runner) struct DyndepPublication {
    /// Advisory lease holding sidecar publication for command consumption.
    lease: DyndepPublicationLease,
}

/// Publish a generated bundle's sidecars before a runner command uses its main file.
///
/// `workspace` is the directory the caller already judged, so publication writes
/// into the same canonical directory the reservation refusal and the reset use
/// rather than resolving the selected path a second time.
pub(in crate::runner) fn materialize_dyndep_bundle(
    workspace: &Workspace,
    bundle: &GeneratedNinja,
) -> Result<DyndepPublication> {
    if bundle.dyndep_files().is_empty() {
        return Ok(DyndepPublication {
            lease: DyndepPublicationLease::empty(),
        });
    }
    let lease = process::materialize_dyndep_files(workspace.dir(), bundle.dyndep_files())?;
    Ok(DyndepPublication { lease })
}

/// Apply retention while respecting a bundle publication lease when available.
///
/// A sidecar-free bundle never opened the dyndep directory, so retention has to
/// establish it here; that path refuses an aliased component instead of
/// resolving through it.
pub(in crate::runner) fn prune_dyndep_bundle(
    workspace: &Workspace,
    current: &[GeneratedDyndep],
    publication: &DyndepPublication,
) -> Result<()> {
    if publication.lease.is_empty() {
        process::prune_dyndep_cache(workspace.dir(), current)?;
    } else {
        publication.lease.prune(current)?;
    }
    Ok(())
}

/// Apply retention through an already-opened working directory.
///
/// Commands that publish through [`materialize_dyndep_bundle`] pass the same
/// [`Workspace`]; the handle form exists for the tool path, which opens its own
/// verified directory and has no workspace value in scope.
pub(in crate::runner) fn prune_dyndep_bundle_in(
    dir: &Dir,
    current: &[GeneratedDyndep],
    publication: &DyndepPublication,
) -> Result<()> {
    if publication.lease.is_empty() {
        process::prune_dyndep_cache(dir, current)?;
    } else {
        publication.lease.prune(current)?;
    }
    Ok(())
}

/// Publish a bundle's sidecars through an already-opened working directory.
///
/// The tool path opens its verified directory for the Ninja invocation and
/// publishes into that same handle.
///
/// # Errors
///
/// Returns an error when the sidecars cannot be materialized.
pub(in crate::runner) fn materialize_dyndep_bundle_in(
    dir: &Dir,
    bundle: &GeneratedNinja,
) -> Result<DyndepPublication> {
    if bundle.dyndep_files().is_empty() {
        return Ok(DyndepPublication {
            lease: DyndepPublicationLease::empty(),
        });
    }
    let lease = process::materialize_dyndep_files(dir, bundle.dyndep_files())?;
    Ok(DyndepPublication { lease })
}
