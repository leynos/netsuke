//! Runner command boundary for generated dyndep sidecar publication.
//!
//! Bundle generation is a query. This module owns the one command that turns a
//! generated bundle into working-directory state, opening ambient authority at
//! the outer runner boundary and injecting the resulting `Dir` into the
//! materializer. Only `runner` command handlers may call this module.

use super::process::{self, DyndepPublicationLease};
use crate::cli::Cli;
use crate::localization::{self, keys};
use crate::ninja_gen::{GeneratedDyndep, GeneratedNinja};
use anyhow::{Context, Result};
use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};

/// Publication lease that protects one serial bundle while a command consumes it.
pub(super) struct DyndepPublication {
    /// Effective Ninja working directory opened for a sidecar-bearing bundle.
    dir: Option<Dir>,
    /// Advisory lease holding sidecar publication for command consumption.
    lease: DyndepPublicationLease,
}

/// Publish a generated bundle's sidecars before a runner command uses its main file.
pub(super) fn materialize_dyndep_bundle(
    cli: &Cli,
    bundle: &GeneratedNinja,
) -> Result<DyndepPublication> {
    if bundle.dyndep_files().is_empty() {
        return Ok(DyndepPublication {
            dir: None,
            lease: DyndepPublicationLease::empty(),
        });
    }
    let dir = open_effective_dir(cli)?;
    let lease = process::materialize_dyndep_files(&dir, bundle.dyndep_files())?;
    Ok(DyndepPublication {
        dir: Some(dir),
        lease,
    })
}

/// Apply retention while respecting a bundle publication lease when available.
pub(super) fn prune_dyndep_bundle(
    cli: &Cli,
    current: &[GeneratedDyndep],
    publication: &DyndepPublication,
) -> Result<()> {
    if let Some(dir) = &publication.dir {
        publication.lease.prune(dir, current)?;
    } else {
        let dir = open_effective_dir(cli)?;
        process::prune_dyndep_cache(&dir, current)?;
    }
    Ok(())
}

/// Open the effective Ninja working directory through the runner capability seam.
fn open_effective_dir(cli: &Cli) -> Result<Dir> {
    if let Some(dir) = &cli.directory {
        let utf8 = Utf8Path::from_path(dir).context(localization::message(
            keys::RUNNER_IO_NON_UTF8_WORKING_DIRECTORY,
        ))?;
        Dir::open_ambient_dir(utf8.as_str(), ambient_authority()).with_context(|| {
            localization::message(keys::RUNNER_IO_OPEN_AMBIENT_DIR).with_arg("path", utf8.as_str())
        })
    } else {
        Dir::open_ambient_dir(".", ambient_authority())
            .context(localization::message(keys::RUNNER_IO_OPEN_AMBIENT_DIR))
    }
}
