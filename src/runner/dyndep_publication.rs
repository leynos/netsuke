//! Runner command boundary for generated dyndep sidecar publication.
//!
//! Bundle generation is a query. This module owns the one command that turns a
//! generated bundle into working-directory state, opening ambient authority at
//! the outer runner boundary and injecting the resulting `Dir` into the
//! materializer. Only `runner` command handlers may call this module.

use super::process;
use crate::cli::Cli;
use crate::localization::{self, keys};
use crate::ninja_gen::GeneratedNinja;
use anyhow::{Context, Result};
use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};

/// Publish a generated bundle's sidecars before a runner command uses its main file.
pub(super) fn materialize_dyndep_bundle(cli: &Cli, bundle: &GeneratedNinja) -> Result<()> {
    if bundle.dyndep_files().is_empty() {
        return Ok(());
    }
    let dir = open_effective_dir(cli)?;
    process::materialize_dyndep_files(&dir, bundle.dyndep_files())
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
