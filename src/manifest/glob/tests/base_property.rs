//! Property tests for injected-base glob expansion through the production
//! [`super::glob_paths`] boundary.
//!
//! The fixed cases in the sibling modules pin individual anchoring shapes.
//! These cover the invariants those shapes are examples of, across arbitrary
//! safe nesting: a relative pattern under `Some(base)` resolves to exactly
//! the fixture files spelled relative to the pattern, two distinct bases
//! never cross-contaminate, an absolute pattern ignores the base, a
//! parent-relative pattern keeps its `..` spelling, `None` retains the
//! unbased behaviour, and produced paths always use forward slashes.
//!
//! Each case builds a disposable temporary fixture tree and invokes the
//! production `glob_paths` function; the environment and working directory of
//! the test process are never mutated.

use super::super::glob_paths;
use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use minijinja::ErrorKind;
use proptest::collection;
use proptest::prelude::*;
use std::collections::BTreeSet;
use tempfile::tempdir;
use test_support::fs as test_fs;

/// Generate a small tree of safe (lowercase ASCII) path segments.
fn segments() -> impl Strategy<Value = Vec<String>> {
    collection::vec("[a-z]{1,5}", 0..4)
}

/// The expected pattern-relative spelling of `segments` joined to `leaf.txt`.
fn expected_relative(segments: &[String]) -> String {
    let mut path = String::new();
    for segment in segments {
        if !path.is_empty() {
            path.push('/');
        }
        path.push_str(segment);
    }
    if !path.is_empty() {
        path.push('/');
    }
    path.push_str("leaf.txt");
    path
}

/// Install `segments/leaf.txt` under `root` and return the expected spelling.
fn install_leaf(root: &std::path::Path, segments: &[String]) -> Result<String> {
    let mut dir = root.to_path_buf();
    for segment in segments {
        dir = dir.join(segment);
        test_fs::create_dir(&dir).with_context(|| format!("create {dir:?}"))?;
    }
    let leaf = dir.join("leaf.txt");
    test_fs::write(&leaf, "x").with_context(|| format!("write {leaf:?}"))?;
    Ok(expected_relative(segments))
}

proptest! {
    /// A relative pattern under `Some(base)` returns exactly the fixture
    /// files spelled relative to the pattern, independent of the base's
    /// absolute location.
    #[test]
    fn relative_pattern_under_base_returns_pattern_relative_paths(
        segments in segments(),
    ) {
        let temp =
            tempdir().context("create a temporary directory").expect("temp dir must be creatable");
        let expected = install_leaf(temp.path(), &segments).expect("fixture must install");
        let base = Utf8Path::from_path(temp.path()).expect("temp paths are UTF-8");
        let mut results = glob_paths("**/*.txt", Some(base)).expect("relative glob must succeed");
        results.sort();
        let mut want = vec![expected];
        want.sort();
        prop_assert_eq!(results, want);
    }

    /// The same relative pattern under two distinct bases returns only each
    /// base's own files; the bases never cross-contaminate.
    #[test]
    fn distinct_bases_do_not_cross_contaminate(
        first in segments(),
        second in segments(),
    ) {
        let temp =
            tempdir().context("create a temporary directory").expect("temp dir must be creatable");
        let base_a = temp.path().join("a");
        test_fs::create_dir(&base_a).expect("base A dir must be creatable");
        let base_b = temp.path().join("b");
        test_fs::create_dir(&base_b).expect("base B dir must be creatable");
        let expected_a = install_leaf(&base_a, &first).expect("fixture A must install");
        let expected_b = install_leaf(&base_b, &second).expect("fixture B must install");
        let pattern = "**/*.txt";

        let results_a =
            glob_paths(pattern, Some(Utf8Path::from_path(&base_a).expect("UTF-8")))
                .expect("glob under base A must succeed");
        let results_b =
            glob_paths(pattern, Some(Utf8Path::from_path(&base_b).expect("UTF-8")))
                .expect("glob under base B must succeed");
        prop_assert_eq!(
            results_a.into_iter().collect::<BTreeSet<_>>(),
            BTreeSet::from([expected_a.clone()]),
            "base A must not see base B's files"
        );
        prop_assert_eq!(
            results_b.into_iter().collect::<BTreeSet<_>>(),
            BTreeSet::from([expected_b]),
            "base B must not see base A's files"
        );
    }

    /// An absolute pattern under `Some(base)` is not anchored: the base is
    /// neither prepended nor stripped, and the pattern-relative suffix is
    /// retained.
    #[test]
    fn absolute_pattern_ignores_the_base(nested in segments()) {
        let temp =
            tempdir().context("create a temporary directory").expect("temp dir must be creatable");
        let concrete = temp.path().join("concrete");
        test_fs::create_dir(&concrete).expect("concrete dir must be creatable");
        let expected = install_leaf(&concrete, &nested).expect("fixture must install");
        let absolute_pattern = format!("{}/**/*.txt", concrete.display());
        // A decoy base that must neither be joined nor stripped.
        let decoy = temp.path().join("decoy");
        test_fs::create_dir(&decoy).expect("decoy dir must be creatable");
        test_fs::write(decoy.join("stray.txt"), "s").expect("decoy file must be writable");
        let found = glob_paths(
            &absolute_pattern,
            Some(Utf8Path::from_path(&decoy).expect("UTF-8")),
        )
        .expect("absolute glob must succeed");
        let results = found.into_iter().collect::<BTreeSet<_>>();
        prop_assert!(
            results.len() == 1,
            "absolute pattern must match only concrete's files: {results:?}"
        );
        let got = results.iter().next().expect("one result was asserted above");
        // Compare the suffix after the concrete base directory.
        let suffix = format!("/{expected}");
        prop_assert!(
            got.ends_with(&suffix),
            "absolute result {got:?} must retain suffix {suffix:?}"
        );
    }
}

/// A parent-relative pattern keeps its `..` spelling in the result.
///
/// The base's parent — here the temporary directory — is isolated, so the
/// result is deterministic; this is the same contract the integration tests
/// pin through a manifest workspace root.
#[test]
fn parent_relative_pattern_preserves_dot_dot() -> Result<()> {
    let temp = tempdir()?;
    let sub = temp.path().join("sub");
    test_fs::create_dir(&sub)?;
    test_fs::write(temp.path().join("out.txt"), "out")?;

    let results = glob_paths(
        "../*.txt",
        Some(Utf8Path::from_path(&sub).expect("temp paths are UTF-8")),
    )?;
    ensure!(
        results == vec!["../out.txt".to_owned()],
        "expected the parent-relative match, got {results:?}"
    );
    Ok(())
}

/// `None` retains the unbased behaviour: an absolute pattern returns absolute
/// paths with no base stripping.
#[test]
fn none_base_keeps_absolute_results() -> Result<()> {
    let temp = tempdir()?;
    let concrete = temp.path().join("concrete");
    test_fs::create_dir(&concrete)?;
    test_fs::write(concrete.join("leaf.txt"), "x")?;

    let pattern = format!("{}/leaf.txt", concrete.display());
    let results = glob_paths(&pattern, None)?;
    ensure!(
        results == vec![pattern.clone()],
        "None must keep the absolute spelling, got {results:?}"
    );
    Ok(())
}

/// Every produced path uses forward slashes on every platform.
#[test]
fn results_use_forward_slashes() -> Result<()> {
    let temp = tempdir()?;
    let base = temp.path().join("base");
    test_fs::create_dir(&base)?;
    let nested = base.join("nested");
    test_fs::create_dir(&nested)?;
    test_fs::write(nested.join("leaf.txt"), "x")?;
    let pattern = "**/*.txt";
    let results = glob_paths(
        pattern,
        Some(Utf8Path::from_path(&base).expect("temp paths are UTF-8")),
    )?;
    ensure!(
        results == vec!["nested/leaf.txt".to_owned()],
        "expected forward-slash spelling, got {results:?}"
    );
    Ok(())
}

/// Assert that a metacharacter in an injected base remains literal.
fn assert_injected_base_metacharacter_is_literal(base_name: &str, decoy_name: &str) -> Result<()> {
    let temp = tempdir()?;
    let base = temp.path().join(base_name);
    let decoy = temp.path().join(decoy_name);
    test_fs::create_dir(&base)?;
    test_fs::create_dir(&decoy)?;
    test_fs::write(base.join("wanted.txt"), "wanted")?;
    test_fs::write(decoy.join("decoy.txt"), "decoy")?;

    let results = glob_paths(
        "*.txt",
        Some(Utf8Path::from_path(&base).context("temporary paths must be UTF-8")?),
    )?;
    ensure!(
        results == vec!["wanted.txt".to_owned()],
        "base {base_name:?} must not match decoy {decoy_name:?}: {results:?}"
    );
    Ok(())
}

/// Treat glob metacharacters in an injected base as literal path components.
///
/// Each neighbouring decoy would match `*.txt` only if the base were compiled
/// as glob syntax instead of being escaped before the user pattern is joined.
#[cfg(unix)]
#[rstest::rstest]
#[case("literal*base", "literalxbase")]
#[case("literal?base", "literalxbase")]
#[case("literal[ab]base", "literalabase")]
#[case("literal{a,b}base", "literalabase")]
fn injected_base_metacharacters_are_literal(
    #[case] base_name: &str,
    #[case] decoy_name: &str,
) -> Result<()> {
    assert_injected_base_metacharacter_is_literal(base_name, decoy_name)
}

/// Cover injected-base metacharacters that Windows permits in directory names.
///
/// Windows reserves `*` and `?` in filesystem components, so the Unix-only
/// cases above retain that matcher coverage while this platform exercises the
/// legal bracket and brace spellings against real directories.
#[cfg(not(unix))]
#[rstest::rstest]
#[case("literal[ab]base", "literalabase")]
#[case("literal{a,b}base", "literalabase")]
fn injected_base_metacharacters_are_literal(
    #[case] base_name: &str,
    #[case] decoy_name: &str,
) -> Result<()> {
    assert_injected_base_metacharacter_is_literal(base_name, decoy_name)
}

/// Propagate a missing injected base as the glob I/O error rather than
/// silently searching from an unrelated fallback directory.
#[test]
fn missing_injected_base_is_an_io_error() -> Result<()> {
    let temp = tempdir()?;
    let missing = temp.path().join("missing");
    let error = glob_paths(
        "*.txt",
        Some(Utf8Path::from_path(&missing).expect("temp paths are UTF-8")),
    )
    .expect_err("missing injected base must not fall back to another directory");
    ensure!(
        error.kind() == ErrorKind::InvalidOperation,
        "missing base must preserve the glob I/O error policy, got {error:?}"
    );
    Ok(())
}
