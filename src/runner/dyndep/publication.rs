//! Runner command boundary for generated dyndep sidecar publication.
//!
//! Bundle generation is a query. This module owns the one command that turns a
//! generated bundle into working-directory state, opening ambient authority at
//! the outer runner boundary and injecting the resulting `Dir` into the
//! materializer. Only `runner` command handlers may call this module.

use super::super::process::{self, DyndepPublicationLease};
use crate::cli::Cli;
use crate::localization::{self, keys};
use crate::ninja_gen::{GeneratedDyndep, GeneratedNinja};
use anyhow::{Context, Result};
use cap_std::{ambient_authority, fs_utf8::Dir};

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
pub(in crate::runner) fn materialize_dyndep_bundle(
    cli: &Cli,
    bundle: &GeneratedNinja,
) -> Result<DyndepPublication> {
    if bundle.dyndep_files().is_empty() {
        return Ok(DyndepPublication {
            lease: DyndepPublicationLease::empty(),
        });
    }
    let dir = open_effective_dir(cli)?;
    let lease = process::materialize_dyndep_files(&dir, bundle.dyndep_files())?;
    Ok(DyndepPublication { lease })
}

/// Apply retention while respecting a bundle publication lease when available.
///
/// A sidecar-free bundle never opened the dyndep directory, so retention has to
/// establish it here; that path refuses an aliased component instead of
/// resolving through it.
pub(in crate::runner) fn prune_dyndep_bundle(
    cli: &Cli,
    current: &[GeneratedDyndep],
    publication: &DyndepPublication,
) -> Result<()> {
    if publication.lease.is_empty() {
        let dir = open_effective_dir(cli)?;
        process::prune_dyndep_cache(&dir, current)?;
    } else {
        publication.lease.prune(current)?;
    }
    Ok(())
}

/// Open the effective Ninja working directory through the runner capability seam.
fn open_effective_dir(cli: &Cli) -> Result<Dir> {
    cli.directory.as_ref().map_or_else(
        || {
            Dir::open_ambient_dir(".", ambient_authority())
                .context(localization::message(keys::RUNNER_IO_OPEN_AMBIENT_DIR))
        },
        |dir| {
            Dir::open_ambient_dir(dir.as_str(), ambient_authority()).with_context(|| {
                localization::message(keys::RUNNER_IO_OPEN_AMBIENT_DIR)
                    .with_arg("path", dir.as_str())
            })
        },
    )
}
