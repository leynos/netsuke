//! Name the graph paths that would collide with Netsuke's generated state.
//!
//! Netsuke writes files under reserved namespaces such as `.netsuke/phony`,
//! and Ninja compares paths textually, so a manifest must not name one of
//! those files. This module holds the reservation itself: the namespace
//! constants every writer shares, and a check over a spelling as the graph
//! states it.
//!
//! The check is deliberately pure. Emitting a Ninja file is a translation of
//! the graph, so generation folds `.` and `..` components lexically and reads
//! nothing from disk. That catches a variant such as `./.netsuke/phony/force`
//! but not an absolute path or a `..` that climbs out of the working directory
//! and back into it, which only that directory can settle. The command layer
//! that owns build state resolves those spellings; it lives with the runner
//! rather than here, so this module cannot reach the filesystem by accident.
//! Case is folded too, because a case-insensitive volume treats
//! `.NETSUKE/PHONY` as the reserved directory itself.

use camino::{Utf8Component, Utf8Path};

/// Reserved namespace for serial-dependency gate paths.
pub(crate) const SERIAL_NAMESPACE: &str = ".netsuke/serial";
/// Reserved namespace for dyndep sidecar files.
pub(crate) const DYNDEP_NAMESPACE: &str = ".netsuke/dyndep";
/// Reserved namespace for hidden phony outputs and the shared force node.
pub(crate) const PHONY_NAMESPACE: &str = ".netsuke/phony";

/// Namespaces whose contents are generated state rather than user outputs.
const RESERVED_NAMESPACES: &[&str] = &[SERIAL_NAMESPACE, DYNDEP_NAMESPACE, PHONY_NAMESPACE];

/// Return whether a relative spelling names a location inside a reserved
/// namespace.
///
/// `.`, empty and `..` components are folded lexically, so the answer depends
/// only on the spelling. An absolute path, a path with a prefix, or a path that
/// climbs above its own root is not reserved here: the graph alone cannot say
/// where it lands, and the command layer answers that against the working
/// directory instead.
pub(crate) fn is_reserved_path(path: &Utf8Path) -> bool {
    normalize_lexically(path).is_some_and(|normalized| is_reserved_spelling(normalized.as_str()))
}

/// Return whether a `/`-separated relative spelling names generated state.
///
/// ASCII case is folded because the volume may not distinguish it. On NTFS or
/// APFS, `.NETSUKE/PHONY/force` names the same entry as the lowercase
/// spelling, so a case-sensitive comparison would let a declaration through and
/// then have the build remove it as stray generated state. Refusing a case
/// variant on a case-sensitive volume as well is the fail-closed trade-off.
fn is_reserved_spelling(candidate: &str) -> bool {
    let folded = candidate.to_ascii_lowercase();
    RESERVED_NAMESPACES
        .iter()
        .any(|namespace| is_within(&folded, namespace))
}

/// Return whether `candidate` equals `namespace` or lies beneath it.
fn is_within(candidate: &str, namespace: &str) -> bool {
    candidate == namespace
        || candidate
            .strip_prefix(namespace)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

/// Resolve `.` and empty components and fold `..` where it can be applied.
///
/// The result joins the remaining components with `/` rather than rebuilding a
/// path through the platform's path type. `Utf8PathBuf::push` inserts the
/// native separator, so on Windows it would produce `.netsuke\phony\force`,
/// which never matches the slash-separated reserved namespaces; the join keeps
/// the comparison spelling fixed. On Unix a backslash is an ordinary filename
/// character and survives as one.
///
/// Returns `None` when a leading `..` climbs above the path's root, or when the
/// path is absolute or carries a prefix.
fn normalize_lexically(path: &Utf8Path) -> Option<String> {
    let mut segments: Vec<&str> = Vec::new();
    for component in path.components() {
        match component {
            Utf8Component::Normal(segment) => segments.push(segment),
            Utf8Component::CurDir => {}
            Utf8Component::ParentDir => {
                segments.pop()?;
            }
            Utf8Component::Prefix(_) | Utf8Component::RootDir => return None,
        }
    }
    Some(segments.join("/"))
}

#[cfg(test)]
mod tests;
