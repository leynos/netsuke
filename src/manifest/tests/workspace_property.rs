//! Property tests for manifest workspace-root resolution.
//!
//! The fixed cases in `workspace.rs` pin a few shapes; these cover the
//! invariants those shapes are examples of, across arbitrary relative and
//! absolute parents and optional bases: an absolute parent wins outright, a
//! relative parent joins onto an absolute base verbatim, and a missing or
//! relative base is resolved against the process working directory so the
//! workspace root always ends up absolute.
//!
//! Absolute parents and bases are anchored at `env::current_dir()`. On
//! Windows a bare leading separator such as `\a` is rooted but not absolute —
//! it has no drive prefix — so `Path::is_absolute()` reports false and the
//! resolver would treat it as relative. Anchoring at the current directory
//! gives every platform a genuinely absolute `D:\...`-style (or `/...`)
//! anchor.

use super::super::workspace::resolve_absolute_workspace_root;
use camino::Utf8PathBuf;
use proptest::{prelude::*, test_runner::TestCaseError};
use std::path::{MAIN_SEPARATOR_STR, Path};

/// A path component: letters only, so no separators or `.`/`..` components.
fn component() -> impl Strategy<Value = String> {
    "[a-z]{1,6}".prop_map(String::from)
}

/// Join generated components with the platform separator.
fn joined(parts: &[String]) -> String {
    parts.join(MAIN_SEPARATOR_STR)
}

/// The process working directory as an absolute UTF-8 anchor.
///
/// Genuinely absolute parents and bases hang off this anchor so the generated
/// paths are absolute on every platform, including Windows where a bare
/// leading separator is rooted but not absolute.
fn absolute_anchor() -> Result<Utf8PathBuf, TestCaseError> {
    let cwd = std::env::current_dir()
        .map_err(|err| TestCaseError::fail(format!("read the working directory: {err}")))?;
    Utf8PathBuf::from_path_buf(cwd)
        .map_err(|_| TestCaseError::fail("the working directory must be valid UTF-8"))
}

proptest! {
    /// An absolute parent wins: the base is irrelevant and the result keeps the
    /// parent verbatim.
    #[test]
    fn absolute_parent_ignores_the_base(
        parent_parts in proptest::collection::vec(component(), 1..4),
        base_kind in 0u8..3,
        base_parts in proptest::collection::vec(component(), 0..3),
    ) {
        let anchor = absolute_anchor()?;
        let parent = anchor.join(joined(&parent_parts));
        let base_text = base_parts.join(MAIN_SEPARATOR_STR);
        let base = match base_kind {
            0 => None,
            1 => Some(Path::new(".")),
            _ => Some(Path::new(&base_text)),
        };
        let resolved =
            resolve_absolute_workspace_root(&parent, base).expect("absolute parent resolves");
        prop_assert_eq!(resolved, parent, "an absolute parent must not be re-anchored");
    }

    /// A relative parent joined onto an absolute base reproduces base.join(..).
    #[test]
    fn relative_parent_joins_onto_an_absolute_base(
        base_parts in proptest::collection::vec(component(), 1..3),
        parent_parts in proptest::collection::vec(component(), 1..4),
    ) {
        let anchor = absolute_anchor()?;
        let base = anchor.join(joined(&base_parts));
        let parent = Utf8PathBuf::from(joined(&parent_parts));
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
        parent_parts in proptest::collection::vec(component(), 1..4),
        base_kind in 0u8..3,
        base_parts in proptest::collection::vec(component(), 0..3),
    ) {
        let parent = Utf8PathBuf::from(joined(&parent_parts));
        let base_text = base_parts.join(MAIN_SEPARATOR_STR);
        let base = match base_kind {
            0 => None,
            1 => Some(Path::new(".")),
            _ => Some(Path::new(&base_text)),
        };
        let resolved =
            resolve_absolute_workspace_root(&parent, base).expect("relative parent resolves");
        prop_assert!(resolved.is_absolute(), "root {resolved:?} should be absolute");
    }
}
