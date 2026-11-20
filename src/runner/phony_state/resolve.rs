//! Decide whether a declared path resolves into reserved generated state.
//!
//! Generation folds a spelling lexically, which is all a pure translation may
//! do. An absolute path, a `..` that leaves the working directory and returns,
//! or a symbolic-link alias names generated state only once the filesystem
//! answers what it reaches, so that question is asked here, in the command
//! layer, and only for a build that already found something to remove.
//!
//! Resolution reads and writes nothing. Every failure other than an absent
//! component is reported rather than folded into "not reserved": a directory
//! that cannot be read must stop the reset, because a guard that fails open
//! would delete the very output it exists to protect.

use super::inspect_context;
use crate::ninja_gen::reserved_paths::is_reserved_path;
use anyhow::{Context, Result};
use camino::{Utf8Component, Utf8Path, Utf8PathBuf};
use std::io::ErrorKind;

/// Return whether `path`, resolved against `workspace`, names generated state.
///
/// Text alone cannot answer this for every spelling: `../project/force` and
/// `/abs/project/force` may name the very node about to be removed, and an
/// alias through a symbolic link names it without saying so. Two resolutions
/// run, because they answer different questions:
///
/// - The whole path, following its final component when that is a link, says
///   whether the declaration reaches into the namespace at all.
/// - The parent alone, keeping the final component as written, says whether the
///   entry about to be removed is the one declared. Following the link there
///   would describe the link's target instead of the entry, so a declared
///   absolute alias whose link points outside the workspace would otherwise
///   escape the check.
///
/// Both use the longest existing ancestor, canonicalized against `workspace`,
/// with the components that do not exist yet folded onto it. The results are
/// given to the same reservation rule generation applies.
///
/// Canonicalization only reads: the removal itself still goes through the
/// verified namespace handles, so this check can never redirect a deletion.
///
/// # Errors
///
/// Returns an error when a path cannot be canonicalized for any reason other
/// than a component that does not exist. Reporting the failure rather than
/// treating the path as unreserved keeps the guard from weakening silently
/// when the filesystem cannot answer.
pub(super) fn resolves_into_reserved_state(workspace: &Utf8Path, path: &Utf8Path) -> Result<bool> {
    if is_reserved_path(path) {
        return Ok(true);
    }
    let target = resolved_target(workspace, path)?;
    let entry = declared_entry(workspace, path)?;
    Ok(within_namespace(workspace, target.as_deref())
        || within_namespace(workspace, entry.as_deref()))
}

/// Resolve `path`'s parent and keep its final component as written.
///
/// A path with no final component — one that ends in `..` or in a separator —
/// names no entry, so there is nothing separate from the full resolution in
/// [`resolves_into_reserved_state`].
fn declared_entry(workspace: &Utf8Path, path: &Utf8Path) -> Result<Option<Utf8PathBuf>> {
    let Some(parent) = path.parent() else {
        return Ok(None);
    };
    let Some(name) = path.file_name() else {
        return Ok(None);
    };
    let resolved = resolved_target(workspace, parent)?;
    Ok(resolved.map(|directory| directory.join(name)))
}

/// Report whether an already-resolved path names reserved state in `workspace`.
fn within_namespace(workspace: &Utf8Path, resolved: Option<&Utf8Path>) -> bool {
    resolved
        .and_then(|candidate| candidate.strip_prefix(workspace).ok())
        .is_some_and(is_reserved_path)
}

/// Resolve `path` against the workspace, folding the components not yet on disk.
///
/// The walk up to the longest existing ancestor is what makes a `..` that
/// re-enters the workspace visible: the spelling alone cannot say where it
/// lands once the filesystem has resolved each component.
///
/// # Errors
///
/// Returns an error when canonicalizing an ancestor fails for any reason other
/// than [`ErrorKind::NotFound`].
fn resolved_target(workspace: &Utf8Path, path: &Utf8Path) -> Result<Option<Utf8PathBuf>> {
    let candidate = workspace.join(path);
    let mut ancestor = candidate.as_path();
    loop {
        match ancestor.canonicalize_utf8() {
            Ok(canonical) => {
                let Ok(tail) = candidate.strip_prefix(ancestor) else {
                    return Ok(None);
                };
                return Ok(fold_tail(&canonical, tail));
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {
                let Some(parent) = ancestor.parent() else {
                    return Ok(None);
                };
                ancestor = parent;
            }
            Err(error) => return Err(error).context(inspect_context(path)),
        }
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
