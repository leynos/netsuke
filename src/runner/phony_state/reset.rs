//! Open the reserved namespace safely and remove a stray force node.
//!
//! Ninja reads the force node's absence as "permanently dirty". A regular
//! file, a directory, or a symbolic link at that path makes the node look
//! present, so `always` and executable `phony` work silently stops running.
//! Clearing it is the command layer's job, because emitting the Ninja file must
//! stay a pure translation of the graph.
//!
//! The path sits in a namespace reserved for generated state, so removing it
//! never deletes a manifest output — provided the namespace is the real
//! directory that reservation implies. A symbolic link there could send the
//! removal anywhere, so the namespace is opened without following links and the
//! reset is refused when either component is not a real directory. The removal
//! reports what it removed and leaves the logging to its caller.

use super::policy::{ReservedStateChecked, verify_reservation};
use super::workspace::{Workspace, inspect_context};
use crate::localization::{self, keys};
use anyhow::{Context, Error, Result};
use camino::{Utf8Path, Utf8PathBuf};
use cap_primitives::fs::{FollowSymlinks, open_dir_nofollow, stat};
use cap_std::fs_utf8::Dir;
use std::io::{self, ErrorKind};
use std::path::Path;

/// Parent directory of the namespace reserved for generated phony state.
pub(super) const NAMESPACE_PARENT: &str = ".netsuke";
/// Leaf directory of that namespace, which holds the force node.
pub(super) const NAMESPACE_LEAF: &str = "phony";
/// Entry inside the namespace occupied by the shared force node.
pub(super) const FORCE_ENTRY: &str = "force";

/// What a reset did.
#[derive(Debug)]
pub(in crate::runner) enum ResetOutcome {
    /// Nothing occupied the force node, so nothing was removed.
    AlreadyClear,
    /// A stray path occupied the node and was removed.
    Removed {
        /// The entry that was removed, relative to the namespace.
        entry: Utf8PathBuf,
    },
}

/// Remove a stray path occupying the shared force node.
///
/// The [`ReservedStateChecked`] argument is the refusal's own proof, and its
/// workspace and paths are re-verified here, so a removal through this function
/// cannot destroy a declared output or the selected manifest, and a proof from
/// a different workspace is refused. The caller logs the outcome, so this
/// function only decides and removes.
///
/// # Errors
///
/// Returns an error when the proof belongs to another workspace, when a
/// recorded path now resolves into reserved state, when the reserved namespace
/// is not a real directory, or when a path present at the force node cannot be
/// inspected or removed.
pub(in crate::runner) fn reset_force_node(
    workspace: &Workspace,
    reservation: &ReservedStateChecked,
) -> Result<ResetOutcome> {
    verify_reservation(workspace, reservation)?;
    let root = workspace.dir();
    let parent = Utf8Path::new(NAMESPACE_PARENT);
    let Some(netsuke) = open_verified_dir(root, NAMESPACE_PARENT, parent)? else {
        return Ok(ResetOutcome::AlreadyClear);
    };
    let namespace = parent.join(NAMESPACE_LEAF);
    let Some(phony) = open_verified_dir(&netsuke, NAMESPACE_LEAF, &namespace)? else {
        return Ok(ResetOutcome::AlreadyClear);
    };
    if !force_entry_exists(&phony)? {
        return Ok(ResetOutcome::AlreadyClear);
    }
    remove_path(&phony, Utf8Path::new(FORCE_ENTRY))?;
    Ok(ResetOutcome::Removed {
        entry: Utf8PathBuf::from(FORCE_ENTRY),
    })
}

/// Report whether anything occupies the force node without following a link.
///
/// # Errors
///
/// Returns an error when the entry exists but cannot be inspected.
fn force_entry_exists(phony: &Dir) -> Result<bool> {
    match phony.symlink_metadata(FORCE_ENTRY) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).context(reset_context(Utf8Path::new(FORCE_ENTRY))),
    }
}

/// Open the directory `leaf` beneath `parent` without following a symlink.
///
/// Returns `Ok(None)` when the entry is absent, because a namespace that does
/// not exist holds no stray state. A symbolic link or a non-directory entry is
/// refused: removal beneath such a path could reach a file outside the
/// reserved namespace.
///
/// The open itself refuses a link as the final component — `O_NOFOLLOW` on
/// Unix, a reparse-point check on the opened handle on Windows — so a link
/// swapped in between the inspection and the open makes the open fail rather
/// than yielding a handle that points elsewhere. The inspection that precedes
/// it is still needed: it distinguishes absence from refusal, and it reports a
/// non-directory entry with the namespace's own diagnostic. A test pins the
/// primitive's guarantee, so a dependency change that dropped it would fail
/// here rather than silently weakening the reset.
///
/// # Errors
///
/// Returns an error when the entry cannot be inspected or opened, or when it
/// is not a real directory. `display` names the entry in the diagnostic,
/// because the parent handle alone does not carry its own path.
pub(super) fn open_verified_dir(
    parent: &Dir,
    leaf: &str,
    display: &Utf8Path,
) -> Result<Option<Dir>> {
    let handle = parent
        .as_cap_std()
        .try_clone()
        .with_context(|| inspect_context(display))?
        .into_std_file();
    let entry = match stat(&handle, Path::new(leaf), FollowSymlinks::No) {
        Ok(entry) => entry,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context(inspect_context(display)),
    };
    if entry.is_symlink() || !entry.is_dir() {
        return Err(refuse(display));
    }
    let opened =
        open_dir_nofollow(&handle, Path::new(leaf)).with_context(|| inspect_context(display))?;
    Ok(Some(Dir::from_std_file(opened)))
}

/// Build the refusal for a namespace entry that is not a real directory.
///
/// The reason is an `io::ErrorKind` rather than prose: "not a directory" is
/// standard-library text, so the diagnostic adds no untranslated sentence to
/// the localized context around it.
fn refuse(display: &Utf8Path) -> Error {
    Error::new(io::Error::from(ErrorKind::NotADirectory)).context(inspect_context(display))
}

/// Remove `entry` from `dir`, treating its absence as success.
///
/// The force node's dirtiness is its absence, so only a leftover path needs
/// clearing. A directory is removed with its contents because anything inside
/// the verified namespace is generated state by reservation. `entry` is
/// resolved through the namespace handle, never through a path that could
/// climb back out of it.
///
/// # Errors
///
/// Returns an error when the entry cannot be inspected or removed.
pub(super) fn remove_path(dir: &Dir, entry: &Utf8Path) -> Result<()> {
    let metadata = match dir.symlink_metadata(entry) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error).context(reset_context(entry)),
    };
    let removed = if metadata.is_dir() {
        dir.remove_dir_all(entry)
    } else {
        dir.remove_file(entry)
    };
    removed.with_context(|| reset_context(entry))
}

/// Build the localized context for a force-node removal failure.
fn reset_context(entry: &Utf8Path) -> localization::LocalizedMessage {
    let path: Utf8PathBuf = Utf8Path::new(NAMESPACE_PARENT)
        .join(NAMESPACE_LEAF)
        .join(entry);
    localization::message(keys::RUNNER_IO_PHONY_RESET).with_arg("path", path.as_str())
}
