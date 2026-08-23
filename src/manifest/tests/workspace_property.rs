//! Property tests for manifest workspace-root resolution.
//!
//! The fixed cases in `workspace.rs` pin a few shapes; these cover the
//! invariants those shapes are examples of, across arbitrary relative and
//! absolute parents and optional bases: an absolute parent wins outright, a
//! relative parent joins onto an absolute base verbatim, and a missing or
//! relative base is resolved against the process working directory so the
//! workspace root always ends up absolute.

use super::super::workspace::resolve_absolute_workspace_root;
use camino::Utf8PathBuf;
use proptest::prelude::*;
use std::path::{MAIN_SEPARATOR, Path};

/// A path component: letters only, so no separators or `.`/`..` components.
fn component() -> impl Strategy<Value = String> {
    "[a-z]{1,6}".prop_map(String::from)
}

/// Join generated components with the platform separator.
fn joined(parts: &[String]) -> String {
    parts.join(&MAIN_SEPARATOR.to_string())
}

/// Generate an absolute parent path of one or more components.
fn absolute_parent() -> impl Strategy<Value = Utf8PathBuf> {
    proptest::collection::vec(component(), 1..4)
        .prop_map(|parts| Utf8PathBuf::from(format!("{MAIN_SEPARATOR}{}", joined(&parts))))
}

/// Generate a relative parent path of one or more components.
fn relative_parent() -> impl Strategy<Value = Utf8PathBuf> {
    proptest::collection::vec(component(), 1..4).prop_map(|parts| Utf8PathBuf::from(joined(&parts)))
}

proptest! {
    /// An absolute parent wins: the base is irrelevant and the result keeps the
    /// parent verbatim.
    #[test]
    fn absolute_parent_ignores_the_base(
        parent in absolute_parent(),
        base_kind in 0u8..3,
        base_parts in proptest::collection::vec(component(), 0..3),
    ) {
        let base_text = base_parts.join(&MAIN_SEPARATOR.to_string());
        let base = match base_kind {
            0 => None,
            1 => Some(Path::new(".")),
            _ => Some(Path::new(&base_text)),
        };
        let resolved = resolve_absolute_workspace_root(&parent, base)
            .expect("absolute parent resolves");
        prop_assert_eq!(resolved, parent, "an absolute parent must not be re-anchored");
    }

    /// A relative parent joined onto an absolute base reproduces base.join(..).
    #[test]
    fn relative_parent_joins_onto_an_absolute_base(
        base_parts in proptest::collection::vec(component(), 1..3),
        parent in relative_parent(),
    ) {
        let base =
            Utf8PathBuf::from(format!("{MAIN_SEPARATOR}{}", joined(&base_parts)));
        let resolved = resolve_absolute_workspace_root(&parent, Some(base.as_std_path()))
            .expect("relative parent resolves against an absolute base");
        prop_assert_eq!(
            resolved,
            base.join(&parent),
            "a relative parent must join onto the absolute base verbatim"
        );
    }

    /// Any relative parent resolves to an absolute root for every base shape,
    /// because a missing or relative base is anchored at the working directory.
    #[test]
    fn relative_parent_always_resolves_absolutely(
        parent in relative_parent(),
        base_kind in 0u8..3,
        base_parts in proptest::collection::vec(component(), 0..3),
    ) {
        let base_text = base_parts.join(&MAIN_SEPARATOR.to_string());
        let base = match base_kind {
            0 => None,
            1 => Some(Path::new(".")),
            _ => Some(Path::new(&base_text)),
        };
        let resolved = resolve_absolute_workspace_root(&parent, base)
            .expect("relative parent resolves");
        prop_assert!(resolved.is_absolute(), "root {resolved:?} should be absolute");
    }
}
