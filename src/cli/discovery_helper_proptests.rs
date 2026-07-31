//! Property and table tests for the discovery hashing and path helpers.
//!
//! These cover invariants that fixed cases cannot: that every correlation hash
//! is the same bounded width and charset whatever the input, and that path
//! normalization is idempotent for both existing and absent paths.
//!
//! `DefaultHasher`'s algorithm is explicitly not stable across Rust releases,
//! so nothing here asserts a specific hash value — only width, charset, and
//! determinism within the run.

use super::diagnostics::{path_hash, short_hash};
use super::paths::normalized_path_key;
use anyhow::{Context, Result, ensure};
use proptest::prelude::*;
use rstest::rstest;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

/// Generate arbitrary byte strings, including empty and non-UTF-8 sequences.
fn hash_input() -> impl Strategy<Value = Vec<u8>> {
    proptest::collection::vec(any::<u8>(), 0..256)
}

/// Generate path-like strings from characters that are legal on both platforms.
fn path_string() -> impl Strategy<Value = String> {
    "[A-Za-z0-9._/-]{0,64}"
}

/// Assert `hash` is the bounded correlation identifier the log fields rely on.
fn ensure_bounded_hash(hash: &str) -> Result<()> {
    ensure!(
        hash.len() == 16,
        "hash should always be 16 characters, got {}: {hash}",
        hash.len()
    );
    ensure!(
        hash.chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
        "hash should be lowercase hex: {hash}"
    );
    Ok(())
}

proptest! {
    /// Every hash is 16 lowercase hex characters, whatever the input.
    #[test]
    fn short_hash_is_always_bounded_and_hex(value in hash_input()) {
        let hash = short_hash(&value);
        prop_assert!(ensure_bounded_hash(&hash).is_ok(), "unbounded hash: {hash}");
    }

    /// The same input hashes identically within a run.
    #[test]
    fn short_hash_is_deterministic(value in hash_input()) {
        prop_assert_eq!(short_hash(&value), short_hash(&value));
    }

    /// A hash never echoes its input, so paths cannot leak through the field.
    #[test]
    fn short_hash_does_not_echo_input(value in "[A-Za-z0-9._/-]{8,64}") {
        let hash = short_hash(value.as_bytes());
        prop_assert!(
            !hash.contains(&value),
            "hash {hash} should not contain input {value}"
        );
    }

    /// `path_hash` inherits the same bounds as `short_hash`.
    #[test]
    fn path_hash_is_always_bounded(value in path_string()) {
        let hash = path_hash(Path::new(&value));
        prop_assert!(ensure_bounded_hash(&hash).is_ok(), "unbounded hash: {hash}");
    }

    /// Normalizing an absent path returns it unchanged, so comparison is stable.
    #[test]
    fn normalized_path_key_is_identity_for_absent_paths(value in path_string()) {
        let absent = format!("/nonexistent-netsuke-proptest/{value}");
        prop_assert_eq!(normalized_path_key(&absent), PathBuf::from(&absent));
    }

    /// Normalization is idempotent, so repeated comparison cannot drift.
    #[test]
    fn normalized_path_key_is_idempotent(value in path_string()) {
        let once = normalized_path_key(&value);
        let twice = normalized_path_key(&once.to_string_lossy());
        prop_assert_eq!(once, twice);
    }
}

/// Normalization resolves the non-canonical forms that broke layer comparison.
///
/// Proptest cannot create real symlinks or `..` chains against the filesystem,
/// so these cases exercise the branch that property tests cannot reach.
#[rstest]
#[case::dot_component("existing", ".")]
#[case::parent_component("existing", "../existing")]
fn normalized_path_key_resolves_non_canonical_forms(
    #[case] dir_name: &str,
    #[case] suffix: &str,
) -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let target = temp.path().join(dir_name);
    test_support::fs::create_dir(&target).context("create target dir")?;

    let non_canonical = target.join(suffix);
    let normalized = normalized_path_key(&non_canonical.to_string_lossy());
    let expected = normalized_path_key(&target.to_string_lossy());

    ensure!(
        normalized == expected,
        "non-canonical {non_canonical:?} should normalize to {expected:?}, got {normalized:?}"
    );
    Ok(())
}
