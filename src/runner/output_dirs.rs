//! Prepare the output directories an executable `phony` edge relies on.
//!
//! Ninja creates an edge's output directories immediately before it runs that
//! edge's recipe. An executable `phony` edge no longer names its declared
//! outputs on the statement that runs the recipe: the statement's only output
//! is a hidden path under `.netsuke/phony`, and the declared names are a native
//! `phony` alias. Ninja never prepares directories for a phony alias, so
//! without this step a recipe whose declared output is `build/report` would run
//! in a workspace without `build/` and fail where the previous lowering created
//! it.
//!
//! The work belongs to the command layer, for the same reason the force-node
//! reset does: emitting a Ninja file is a pure translation of the graph, so a
//! generated file that is run by hand keeps Ninja's own behaviour and needs no
//! help. This module holds the one ambient filesystem operation in the runner
//! that capability handles cannot express, because a manifest may declare an
//! output outside the working directory: an absolute path, or a `..` spelling
//! that leaves it. `cap_std` refuses those by design, and the recipe itself
//! already runs with full ambient authority, so creating the directories of the
//! paths the manifest declared grants nothing the build does not already hold.
//! `dylint.toml` records the exemption for exactly this module.

use super::Cli;
use super::path_helpers::canonical_working_directory;
use anyhow::{Context, Result};
use camino::Utf8PathBuf;

/// Create every directory in `dirs`, resolving them against the build root.
///
/// `dirs` holds the parent directories of the outputs of every executable
/// `phony` edge. They are created whether or not this invocation reaches the
/// edge: the alternative would duplicate Ninja's target selection, and an empty
/// directory is the only visible effect.
///
/// # Errors
///
/// Returns an error when the working directory cannot be resolved, or when a
/// directory cannot be created. A component that exists as a file is reported
/// rather than replaced, so a manifest can never turn a user's file into its
/// own directory.
pub(in crate::runner) fn create_output_dirs(cli: &Cli, dirs: &[Utf8PathBuf]) -> Result<()> {
    if dirs.is_empty() {
        return Ok(());
    }
    let root = canonical_working_directory(cli)?;
    for dir in dirs {
        // Joining an absolute or prefixed path replaces the base, matching how
        // Ninja reads the same spelling in the generated file.
        let path = root.join(dir);
        std::fs::create_dir_all(path.as_std_path())
            .with_context(|| format!("creating the output directory {path}"))?;
    }
    Ok(())
}
