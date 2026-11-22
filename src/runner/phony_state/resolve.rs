//! Decide whether a declared path resolves into reserved generated state.
//!
//! Generation folds a spelling lexically, which is all a pure translation may
//! do. An absolute path, a `..` that leaves the working directory and returns,
//! or a symbolic-link alias names generated state only once the filesystem
//! answers what it reaches, so that question is asked here, in the command
//! layer, on every build.
//!
//! A link whose target does not exist yet is resolved too. Canonicalization
//! reports it as absent, but the link itself exists and says where a write
//! would land, so `alias -> .netsuke/phony/force` must be refused before a
//! recipe writes through it and creates reserved state. Link traversal is
//! bounded, and a chain longer than the bound fails closed.
//!
//! Resolution reads and writes nothing. Every failure other than an absent
//! component is reported rather than folded into "not reserved": a directory
//! that cannot be read must stop the reset, because a guard that fails open
//! would delete the very output it exists to protect.

use super::reset::inspect_context;
use crate::ninja_gen::reserved_paths::is_reserved_path;
use anyhow::{Context, Error, Result};
use camino::{Utf8Component, Utf8Path, Utf8PathBuf};
use std::io::ErrorKind;

/// Maximum symbolic links followed for one declared spelling.
///
/// A build path that reaches further than this cannot be written either, so the
/// bound only decides which diagnosis a pathological manifest receives.
const MAX_LINK_HOPS: usize = 40;

/// One step in resolving a spelling against the filesystem.
enum Resolution {
    /// The longest existing prefix resolved; the absent tail was folded on.
    Resolved(Option<Utf8PathBuf>),
    /// The longest existing prefix ends in a link whose target is absent.
    DanglingLink {
        /// The link itself, which still exists.
        link: Utf8PathBuf,
        /// Target the link names, made absolute against the link's directory.
        target: Utf8PathBuf,
    },
}

/// What inspecting one ancestor established.
enum Step {
    /// The ancestor answered the question.
    Done(Resolution),
    /// The ancestor does not exist and is not a link; try its parent.
    Missing,
}

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
    let spelling = cancel_spelling(path);
    if spelling.as_deref().is_some_and(is_reserved_path) {
        return Ok(true);
    }
    let candidate = spelling.as_deref().unwrap_or(path);
    let target = resolved_target(workspace, candidate)?;
    let entry = declared_entry(workspace, candidate)?;
    Ok(within_namespace(workspace, target.as_deref())
        || within_namespace(workspace, entry.as_deref())
        || reserved_ancestor(workspace, candidate)?)
}

/// Cancel `.` and `..` components lexically, as Ninja does when it parses.
///
/// Ninja removes a `..` against the component before it even when that
/// component does not exist. Resolution against the filesystem alone would keep
/// the nonexistent prefix and miss the path Ninja actually uses, so the
/// spelling is cancelled first and only then resolved, the same way Ninja reads
/// it. Returns `None` when a `..` climbs above an absolute root, which names no
/// reachable path.
fn cancel_spelling(path: &Utf8Path) -> Option<Utf8PathBuf> {
    let mut cancelled = Utf8PathBuf::new();
    for component in path.components() {
        match component {
            Utf8Component::Prefix(prefix) => cancelled.push(prefix.as_str()),
            Utf8Component::RootDir => cancelled.push(Utf8Path::new("/")),
            Utf8Component::CurDir => {}
            Utf8Component::Normal(segment) => cancelled.push(segment),
            Utf8Component::ParentDir => {
                if !cancelled.pop() {
                    return None;
                }
            }
        }
    }
    Some(cancelled)
}

/// Return whether an existing ancestor resolves into reserved state.
///
/// A path can pass through the reserved namespace and leave it again through a
/// link, so the entry it finally reaches is ordinary while the reset would
/// still delete the reserved ancestor it passed through. The selected manifest
/// is the case that matters: a build that removed `force/` would delete the
/// link the manifest was read through. Each ancestor is canonicalized, which
/// follows any link on the way to it, and stops at the workspace because
/// nothing above it can be reserved state.
///
/// # Errors
///
/// Returns an error when canonicalizing an ancestor fails for any reason other
/// than an absent entry.
fn reserved_ancestor(workspace: &Utf8Path, path: &Utf8Path) -> Result<bool> {
    let candidate = workspace.join(path);
    let mut ancestor = candidate.parent();
    while let Some(current) = ancestor {
        if !current.starts_with(workspace) {
            return Ok(false);
        }
        match current.canonicalize_utf8() {
            Ok(canonical) => {
                if within_namespace(workspace, Some(&canonical)) {
                    return Ok(true);
                }
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error).context(inspect_context(current)),
        }
        ancestor = current.parent();
    }
    Ok(false)
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
/// A dangling symbolic link also reports its target as absent, so the walk
/// cannot stop at the first absent component: when the longest existing prefix
/// is a link, its target is substituted and resolution continues from there.
/// The hop bound keeps a link cycle or an excessive chain from looping; such a
/// spelling fails closed rather than passing as unrelated.
///
/// # Errors
///
/// Returns an error when canonicalizing an ancestor fails for any reason other
/// than [`ErrorKind::NotFound`], when a link cannot be read, or when the hop
/// bound is exhausted.
fn resolved_target(workspace: &Utf8Path, path: &Utf8Path) -> Result<Option<Utf8PathBuf>> {
    let mut candidate = workspace.join(path);
    for _ in 0..=MAX_LINK_HOPS {
        match resolve_once(&candidate)? {
            Resolution::Resolved(resolved) => return Ok(resolved),
            Resolution::DanglingLink { link, target } => {
                let Ok(tail) = candidate.strip_prefix(&link) else {
                    return Ok(None);
                };
                candidate = join_tail(&target, tail);
            }
        }
    }
    // The bound is the diagnosis: a spelling that still names links after this
    // many substitutions cannot be written, so the build refuses to guess.
    Err(Error::msg(inspect_context(path).to_string()))
}

/// Resolve one spelling until an existing prefix appears or a dangling link does.
///
/// # Errors
///
/// Returns an error when canonicalizing an ancestor fails for any reason other
/// than [`ErrorKind::NotFound`], or when probing a link fails for any reason
/// other than an absent entry or a non-link.
fn resolve_once(candidate: &Utf8Path) -> Result<Resolution> {
    let mut ancestor = candidate;
    loop {
        match inspect_ancestor(candidate, ancestor)? {
            Step::Done(resolution) => return Ok(resolution),
            Step::Missing => {
                let Some(parent) = ancestor.parent() else {
                    return Ok(Resolution::Resolved(None));
                };
                ancestor = parent;
            }
        }
    }
}

/// Inspect one ancestor, folding the absent tail when it exists.
///
/// # Errors
///
/// Returns an error when canonicalizing the ancestor fails for any reason other
/// than an absent entry, or when probing a possible link fails for any reason
/// other than an absent entry or a non-link.
fn inspect_ancestor(candidate: &Utf8Path, ancestor: &Utf8Path) -> Result<Step> {
    match ancestor.canonicalize_utf8() {
        Ok(canonical) => Ok(Step::Done(fold_onto(candidate, ancestor, &canonical))),
        Err(error) if error.kind() == ErrorKind::NotFound => dangling_step(ancestor),
        Err(error) => Err(error).context(inspect_context(candidate)),
    }
}

/// Fold the components below `ancestor` onto its canonical path.
fn fold_onto(candidate: &Utf8Path, ancestor: &Utf8Path, canonical: &Utf8Path) -> Resolution {
    let Ok(tail) = candidate.strip_prefix(ancestor) else {
        return Resolution::Resolved(None);
    };
    Resolution::Resolved(fold_tail(canonical, tail))
}

/// Decide whether an absent ancestor is a link whose target is absent.
///
/// A dangling link reports its target as absent, but reading the link itself
/// succeeds. A missing entry and a non-link both report [`ErrorKind::NotFound`]
/// or [`ErrorKind::InvalidInput`], so the walk continues.
///
/// # Errors
///
/// Returns an error when reading the link fails for any reason other than those
/// two kinds, or when its relative target has no directory to resolve against.
fn dangling_step(ancestor: &Utf8Path) -> Result<Step> {
    match ancestor.read_link_utf8() {
        Ok(target) => Ok(Step::Done(Resolution::DanglingLink {
            link: ancestor.to_path_buf(),
            target: dangling_target(ancestor, target)?,
        })),
        Err(error)
            if error.kind() == ErrorKind::NotFound || error.kind() == ErrorKind::InvalidInput =>
        {
            Ok(Step::Missing)
        }
        Err(error) => Err(error).context(inspect_context(ancestor)),
    }
}

/// Make a dangling link's target absolute against the link's directory.
///
/// # Errors
///
/// Returns an error when the link sits at the filesystem root, so a relative
/// target has no directory to resolve against.
fn dangling_target(link: &Utf8Path, target: Utf8PathBuf) -> Result<Utf8PathBuf> {
    if target.is_absolute() {
        return Ok(target);
    }
    let Some(directory) = link.parent() else {
        return Err(Error::msg(inspect_context(link).to_string()));
    };
    Ok(directory.join(target))
}

/// Append a relative tail to an already-resolved directory.
fn join_tail(directory: &Utf8Path, tail: &Utf8Path) -> Utf8PathBuf {
    if tail.as_str().is_empty() {
        directory.to_path_buf()
    } else {
        directory.join(tail)
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
