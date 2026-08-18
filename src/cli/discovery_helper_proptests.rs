//! Property and table tests for the discovery hashing and path helpers.
//!
//! These cover invariants that fixed cases cannot: that every correlation hash
//! is the same bounded width and charset whatever the input, and that path
//! normalization is idempotent for existing paths while absent paths report an
//! error.
//!
//! `DefaultHasher`'s algorithm is explicitly not stable across Rust releases,
//! so nothing here asserts a specific hash value — only width, charset, and
//! determinism within the run.

use super::diagnostics::{path_hash, short_hash};
use super::layers::collect_file_layers;
use super::paths::{FailingPathNormalizer, FsPathNormalizer, normalized_path_key};
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

fn project_alias(temp: &Path, project_name: &str, spelling: u8) -> PathBuf {
    let project = temp.join(project_name);
    match spelling {
        0 => project,
        1 => project.join("."),
        _ => temp.join(project_name).join("..").join(project_name),
    }
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

    /// A hash field never contains its input verbatim.
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

    /// An absent path reports the failure rather than absorbing it.
    ///
    /// The caller owns the fallback; see `collect_file_layers`.
    #[test]
    fn normalized_path_key_reports_absent_paths(value in path_string()) {
        let absent = format!("/nonexistent-netsuke-proptest/{value}");
        prop_assert!(normalized_path_key(&FsPathNormalizer, &absent).is_err());
    }

    /// Normalization is idempotent, so repeated comparison cannot drift.
    #[test]
    fn normalized_path_key_is_idempotent(
        value in "[A-Za-z0-9][A-Za-z0-9._-]{0,63}"
    ) {
        let temp = tempdir().expect("create temp dir for resolvable path");
        let path = temp.path().join(value);
        test_support::fs::create_dir(&path).expect("create generated directory");
        let once = normalized_path_key(&FsPathNormalizer, &path.to_string_lossy())
            .expect("generated path must normalize");
        let twice = normalized_path_key(&FsPathNormalizer, &once.to_string_lossy())
            .expect("an already-resolved path must normalize again");
        prop_assert_eq!(once, twice);
    }

    /// Every generated spelling of one project directory produces one layer.
    #[test]
    fn project_config_aliases_have_one_canonical_layer(
        project_name in "[A-Za-z0-9][A-Za-z0-9_-]{0,31}",
        spelling in 0_u8..3,
    ) {
        let temp = tempdir().expect("create temp dir for project alias");
        let project = temp.path().join(&project_name);
        test_support::fs::create_dir(&project).expect("create generated project directory");
        let config = project.join(".netsuke.toml");
        test_support::fs::write(&config, "default_targets = [\"alpha\"]\n")
            .expect("write generated project config");

        let layers = collect_file_layers(Some(&project_alias(temp.path(), &project_name, spelling)))
            .expect("discover generated project config");
        let discovered = layers
            .iter()
            .filter_map(|layer| layer.path().map(|path| path.as_str().to_owned()))
            .collect::<Vec<_>>();
        let canonical = normalized_path_key(&FsPathNormalizer, &config.to_string_lossy())
            .expect("canonicalize generated project config")
            .to_string_lossy()
            .into_owned();

        prop_assert_eq!(discovered, vec![canonical]);
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
    let normalized = normalized_path_key(&FsPathNormalizer, &non_canonical.to_string_lossy())
        .context("normalize non-canonical path")?;
    let expected = normalized_path_key(&FsPathNormalizer, &target.to_string_lossy())
        .context("normalize target path")?;

    ensure!(
        normalized == expected,
        "non-canonical {non_canonical:?} should normalize to {expected:?}, got {normalized:?}"
    );
    Ok(())
}

/// Normalization follows a project alias into a different directory.
///
/// Discovery accepts user-supplied paths, so its capability root must cover
/// the full absolute path rather than rejecting a symlink that leaves the
/// alias's parent directory.
#[cfg(unix)]
#[test]
fn normalized_path_key_follows_cross_directory_symlinks() -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let target = temp.path().join("project");
    let aliases = temp.path().join("aliases");
    test_support::fs::create_dir(&target).context("create project dir")?;
    test_support::fs::create_dir(&aliases).context("create aliases dir")?;

    let alias = aliases.join("project-link");
    test_support::fs::symlink(&target, &alias).context("create project alias")?;

    let normalized = normalized_path_key(&FsPathNormalizer, &alias.to_string_lossy())
        .context("normalize project alias")?;
    let expected = normalized_path_key(&FsPathNormalizer, &target.to_string_lossy())
        .context("normalize project path")?;

    ensure!(
        normalized == expected,
        "project alias {alias:?} should normalize to {expected:?}, got {normalized:?}"
    );
    Ok(())
}

/// `normalized_path_key` propagates the normalizer's error unchanged.
#[test]
fn normalized_path_key_propagates_normalizer_failure() -> Result<()> {
    let error = normalized_path_key(&FailingPathNormalizer, "/any/path")
        .expect_err("the failing normalizer must surface its error");
    ensure!(
        error.kind() == std::io::ErrorKind::PermissionDenied,
        "expected the normalizer's own error kind, got {error:?}"
    );
    Ok(())
}
