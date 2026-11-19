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
//! deletes a manifest output — provided the namespace is the real directory
//! that reservation implies. A symbolic link there could send the removal
//! anywhere, so the namespace is opened without following links and the reset
//! is refused when either component is not a real directory.
//!
//! The removal also refuses when a manifest path resolves into the namespace.
//! Generation rejects the literal and `./`-prefixed spellings, but an absolute
//! path, a `..` path that re-enters the workspace, or a symbolic-link alias can
//! only be judged against the working directory. Deleting the entry under such
//! a spelling would destroy the very output the manifest declared, so the
//! build stops with the reservation's own diagnostic instead. That resolution
//! lives here rather than in generation, because reading the filesystem is the
//! command layer's work: generation stays a pure translation of the graph.

use super::path_helpers::{canonical_working_directory, open_effective_dir};
use crate::cli::Cli;
use crate::localization::{self, keys};
use crate::ninja_gen::reserved_paths::is_reserved_path;
use anyhow::{Context, Error, Result};
use camino::{Utf8Component, Utf8Path, Utf8PathBuf};
use cap_primitives::fs::{FollowSymlinks, open_dir_nofollow, stat};
use cap_std::fs_utf8::Dir;
use std::io::{self, ErrorKind};
use std::path::Path;

/// Parent directory of the namespace reserved for generated phony state.
const NAMESPACE_PARENT: &str = ".netsuke";
/// Leaf directory of that namespace, which holds the force node.
const NAMESPACE_LEAF: &str = "phony";
/// Entry inside the namespace occupied by the shared force node.
const FORCE_ENTRY: &str = "force";

/// Remove a stray path occupying the shared force node.
///
/// `declared` holds every path the manifest names, as written. They are
/// checked only when something is present to remove, because an absent force
/// node cannot destroy anything.
///
/// # Errors
///
/// Returns an error when the working directory cannot be opened, when the
/// reserved namespace is not a real directory, when a declared path resolves
/// into a reserved namespace, or when a path present at the force node cannot
/// be inspected or removed.
pub(in crate::runner) fn reset_force_node(cli: &Cli, declared: &[Utf8PathBuf]) -> Result<()> {
    let root = open_effective_dir(cli)?;
    let parent = Utf8Path::new(NAMESPACE_PARENT);
    let Some(netsuke) = open_verified_dir(&root, NAMESPACE_PARENT, parent)? else {
        return Ok(());
    };
    let namespace = parent.join(NAMESPACE_LEAF);
    let Some(phony) = open_verified_dir(&netsuke, NAMESPACE_LEAF, &namespace)? else {
        return Ok(());
    };
    if !force_entry_exists(&phony)? {
        return Ok(());
    }
    let workspace = canonical_working_directory(cli)?;
    if let Some(path) = declared
        .iter()
        .find(|path| resolves_into_reserved_state(&workspace, path))
    {
        return Err(alias_refusal(path));
    }
    remove_path(&phony, Utf8Path::new(FORCE_ENTRY))
}

/// Return whether `path`, resolved against `workspace`, names generated state.
///
/// Text alone cannot answer this for every spelling: `../project/force` and
/// `/abs/project/force` may name the very node about to be removed, and an
/// alias through a symbolic link names it without saying so. The longest
/// existing ancestor is therefore canonicalized against `workspace` — an
/// absolute path with links resolved — and the components that do not exist yet
/// are applied to it. The result is then compared with the workspace and given
/// to the same reservation rule generation applies.
///
/// Canonicalization only reads: the removal itself still goes through the
/// verified namespace handles, so this check can never redirect a deletion.
fn resolves_into_reserved_state(workspace: &Utf8Path, path: &Utf8Path) -> bool {
    is_reserved_path(path)
        || resolved_target(workspace, path)
            .as_deref()
            .and_then(|resolved| resolved.strip_prefix(workspace).ok())
            .is_some_and(is_reserved_path)
}

/// Resolve `path` against the workspace, folding the components not yet on disk.
fn resolved_target(workspace: &Utf8Path, path: &Utf8Path) -> Option<Utf8PathBuf> {
    let candidate = workspace.join(path);
    let mut ancestor = candidate.as_path();
    loop {
        if let Ok(canonical) = ancestor.canonicalize_utf8() {
            let tail = candidate.strip_prefix(ancestor).ok()?;
            return fold_tail(&canonical, tail);
        }
        ancestor = ancestor.parent()?;
    }
}

/// Apply a relative tail to a canonical path.
///
/// Returns `None` when the tail climbs above the filesystem root, which no
/// existing path can satisfy.
fn fold_tail(canonical: &Utf8Path, tail: &Utf8Path) -> Option<Utf8PathBuf> {
    let mut resolved = canonical.to_owned();
    for component in tail.components() {
        match component {
            Utf8Component::Normal(segment) => resolved.push(segment),
            Utf8Component::CurDir => {}
            Utf8Component::ParentDir => resolved = resolved.parent()?.to_owned(),
            Utf8Component::Prefix(_) | Utf8Component::RootDir => return None,
        }
    }
    Some(resolved)
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

/// Build the refusal for a declared path that resolves into reserved state.
///
/// The message is the generator's own reservation diagnostic: this check only
/// extends the same reservation to spellings that need the working directory
/// to resolve, so users see one explanation for one rule.
fn alias_refusal(path: &Utf8Path) -> Error {
    Error::msg(
        localization::message(keys::NINJA_GEN_RESERVED_OUTPUT_PATH)
            .with_arg("path", path.as_str())
            .to_string(),
    )
}

/// Open the directory `leaf` beneath `parent` without following a symlink.
///
/// Returns `Ok(None)` when the entry is absent, because a namespace that does
/// not exist holds no stray state. A symbolic link or a non-directory entry is
/// refused: removal beneath such a path could reach a file outside the
/// reserved namespace. `open_dir_nofollow` alone accepts a directory symlink
/// with the pinned capability implementation, so the entry is inspected
/// without following it first, and opened without following it second. That
/// mirrors the glob walker's literal-prefix check.
///
/// # Errors
///
/// Returns an error when the entry cannot be inspected or opened, or when it
/// is not a real directory. `display` names the entry in the diagnostic,
/// because the parent handle alone does not carry its own path.
fn open_verified_dir(parent: &Dir, leaf: &str, display: &Utf8Path) -> Result<Option<Dir>> {
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

/// Build the localized context for a namespace inspection failure.
fn inspect_context(display: &Utf8Path) -> localization::LocalizedMessage {
    localization::message(keys::RUNNER_IO_PHONY_INSPECT).with_arg("path", display.as_str())
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
fn remove_path(dir: &Dir, entry: &Utf8Path) -> Result<()> {
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
    removed.with_context(|| reset_context(entry))?;
    tracing::warn!(
        path = %entry,
        "removed a stray path from the reserved phony namespace; forced edges stay dirty"
    );
    Ok(())
}

/// Build the localized context for a force-node removal failure.
fn reset_context(entry: &Utf8Path) -> localization::LocalizedMessage {
    let path: Utf8PathBuf = Utf8Path::new(NAMESPACE_PARENT)
        .join(NAMESPACE_LEAF)
        .join(entry);
    localization::message(keys::RUNNER_IO_PHONY_RESET).with_arg("path", path.as_str())
}

#[cfg(test)]
mod tests;
