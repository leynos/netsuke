//! Clear stray state that would stop the shared force node from forcing.
//!
//! Ninja decides whether an input-less `phony` output is dirty by whether that
//! path exists. The force node leans on exactly that rule: while
//! `.netsuke/phony/force` is absent, every edge that depends on it rebuilds.
//! A regular file, a directory, or a symbolic link at that path instead makes
//! the node look present, so `always` and executable `phony` work silently
//! stops running.
//!
//! Generation cannot repair this, because emitting the Ninja file must stay a
//! pure translation of the graph. The build command therefore clears the force
//! node before Ninja starts, so a stray path cannot disable the promise. The
//! path sits in a namespace reserved for generated state, so removing it never
//! deletes a manifest output.

use super::path_helpers::open_effective_dir;
use crate::cli::Cli;
use crate::ninja_gen::FORCE_PATH;
use anyhow::{Context, Result};
use camino::Utf8Path;
use cap_std::fs_utf8::Dir;
use std::io::ErrorKind;

/// Remove a stray path occupying the shared force node.
///
/// # Errors
///
/// Returns an error when the working directory cannot be opened, when the
/// force node cannot be inspected, or when a path present there cannot be
/// removed.
pub(in crate::runner) fn reset_force_node(cli: &Cli) -> Result<()> {
    let dir = open_effective_dir(cli)?;
    remove_path(&dir, Utf8Path::new(FORCE_PATH))
}

/// Remove `path` from `dir`, treating its absence as success.
///
/// The force node's dirtiness is its absence, so only a leftover path needs
/// clearing. A directory is removed with its contents because anything inside
/// `.netsuke/phony` is generated state by reservation.
fn remove_path(dir: &Dir, path: &Utf8Path) -> Result<()> {
    let metadata = match dir.symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).with_context(|| format!("inspect reserved path {path}"));
        }
    };
    let removed = if metadata.is_dir() {
        dir.remove_dir_all(path)
    } else {
        dir.remove_file(path)
    };
    removed.with_context(|| format!("remove stray path {path}"))?;
    tracing::warn!(
        path = %path,
        "removed a stray path from the reserved phony namespace; forced edges stay dirty"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Verify that a stray path at the force node cannot survive a reset.

    use super::remove_path;
    use anyhow::{Context, Result, ensure};
    use camino::Utf8Path;
    use cap_std::{ambient_authority, fs_utf8::Dir};
    use tempfile::TempDir;

    /// Open a fresh temporary directory as a capability handle.
    fn workspace() -> Result<(TempDir, Dir)> {
        let directory = TempDir::new().context("create a temporary workspace")?;
        let path = directory
            .path()
            .to_str()
            .context("workspace path is UTF-8")?;
        let root = Dir::open_ambient_dir(path, ambient_authority())
            .context("open the temporary workspace")?;
        Ok((directory, root))
    }

    #[test]
    fn an_absent_force_node_is_left_absent() -> Result<()> {
        let (_directory, root) = workspace()?;
        remove_path(&root, Utf8Path::new(".netsuke/phony/force"))?;
        ensure!(
            !root
                .try_exists(".netsuke/phony/force")
                .context("check the force node")?,
            "resetting an absent force node must not create it",
        );
        Ok(())
    }

    #[test]
    fn a_regular_file_at_the_force_node_is_removed() -> Result<()> {
        let (_directory, root) = workspace()?;
        root.create_dir_all(".netsuke/phony")
            .context("create the phony namespace")?;
        root.write(".netsuke/phony/force", b"stray")
            .context("write a stray force node")?;
        remove_path(&root, Utf8Path::new(".netsuke/phony/force"))?;
        ensure!(
            !root
                .try_exists(".netsuke/phony/force")
                .context("check the force node")?,
            "a stray file at the force node must be removed",
        );
        Ok(())
    }

    #[test]
    fn a_directory_at_the_force_node_is_removed_with_its_contents() -> Result<()> {
        let (_directory, root) = workspace()?;
        root.create_dir_all(".netsuke/phony/force")
            .context("create a stray directory at the force node")?;
        root.write(".netsuke/phony/force/keep", b"stray")
            .context("write inside the stray directory")?;
        remove_path(&root, Utf8Path::new(".netsuke/phony/force"))?;
        ensure!(
            !root
                .try_exists(".netsuke/phony/force")
                .context("check the force node")?,
            "a stray directory at the force node must be removed",
        );
        Ok(())
    }
}
