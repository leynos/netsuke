//! Shared setup for the force-node reset tests.
//!
//! Every case needs a temporary workspace opened as a capability handle, a
//! stray force node, and a way to run the reset with an injected working
//! directory. Keeping that setup here lets the example tests and the properties
//! describe one workspace the same way instead of each rebuilding it.

use super::super::{
    FORCE_ENTRY, NAMESPACE_LEAF, NAMESPACE_PARENT, open_verified_dir, refuse_reserved_declarations,
    reset_force_node,
};
use crate::cli::Cli;
use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use std::io::ErrorKind;
use tempfile::TempDir;

/// A temporary workspace opened as a capability handle.
pub(super) struct Workspace {
    /// Owns the temporary directory for the test's lifetime.
    _directory: TempDir,
    /// Canonical absolute path of the same directory.
    pub(super) root: Utf8PathBuf,
    /// Capability handle rooted at [`Self::root`].
    pub(super) dir: Dir,
}

/// Create a fresh temporary workspace.
pub(super) fn workspace() -> Result<Workspace> {
    let directory = TempDir::new().context("create a temporary workspace")?;
    let path = Utf8Path::from_path(directory.path()).context("workspace path is UTF-8")?;
    let root = path
        .canonicalize_utf8()
        .context("canonicalize the temporary workspace")?;
    let dir = Dir::open_ambient_dir(&root, ambient_authority())
        .context("open the temporary workspace")?;
    Ok(Workspace {
        _directory: directory,
        root,
        dir,
    })
}

/// Create the phony namespace and return a handle to it.
pub(super) fn namespace(root: &Dir) -> Result<Dir> {
    root.create_dir_all(Utf8Path::new(NAMESPACE_PARENT).join(NAMESPACE_LEAF))
        .context("create the phony namespace")?;
    open_verified_dir(root, NAMESPACE_PARENT, Utf8Path::new(NAMESPACE_PARENT))?
        .context("the namespace parent must exist")?
        .open_dir(NAMESPACE_LEAF)
        .context("open the phony namespace")
}

/// Write a stray entry at the force node.
pub(super) fn stray_force_node(root: &Dir) -> Result<()> {
    namespace(root)?
        .write(FORCE_ENTRY, b"stray")
        .context("write a stray force node")
}

/// Run the reservation refusal, then the reset, as a build does.
///
/// The experiment builds a `Cli` whose working directory is `root`, so a
/// spelling only that directory can resolve reaches the same code the build
/// command runs. The refusal comes first here exactly as it does in
/// `execute_build`, which keeps these cases honest about the ordering the
/// production path relies on.
pub(super) fn reset(root: &Utf8Path, declared: &[Utf8PathBuf]) -> Result<()> {
    let cli = Cli {
        directory: Some(root.to_owned()),
        ..Cli::default()
    };
    refuse_reserved_declarations(&cli, declared)?;
    reset_force_node(&cli)
}

/// Report whether an entry still occupies the force node.
///
/// The check never follows a link: a declared alias of the node is a link, and
/// following it would report the target's presence rather than the entry's.
pub(super) fn force_node_exists(dir: &Dir) -> Result<bool> {
    match namespace(dir)?.symlink_metadata(FORCE_ENTRY) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).context("inspect the force node"),
    }
}
