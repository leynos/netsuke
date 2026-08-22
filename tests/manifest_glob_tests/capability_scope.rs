//! Manifest-level coverage of the capability-scoped expansion.
//!
//! The unit tests drive `glob_paths` directly. These go through the
//! manifest and its Jinja rendering, so they also pin that a skipped or
//! empty expansion reaches `foreach` as an ordinary list rather than
//! surfacing as a parse error.

use super::{manifest_yaml, target_names, temp_dir};
use anyhow::{ensure, Context, Result};
use rstest::rstest;
use std::path::Path;

/// Build a manifest with one `foreach` target per glob match.
fn glob_manifest(pattern: &str) -> String {
    manifest_yaml(&format!(
        concat!(
            "targets:\n",
            "  - foreach: glob({pattern:?})\n",
            "    name: \"{{{{ item }}}}\"\n",
            "    command: echo hi\n",
        ),
        pattern = pattern,
    ))
}

/// A prefix that names nothing, and one that names a file, both expand to
/// no targets rather than failing the manifest.
#[rstest]
#[case::missing_directory("no-such-dir")]
#[case::regular_file("a.txt")]
fn unopenable_prefix_yields_no_targets(
    temp_dir: tempfile::TempDir,
    #[case] prefix: &str,
) -> Result<()> {
    test_support::fs::write(temp_dir.path().join("a.txt"), "a")?;
    let fixture_dir: &Path = temp_dir.path();
    let pattern_path = fixture_dir.join(prefix).join("*.txt");
    let pattern = pattern_path
        .to_str()
        .context("glob test pattern must be valid UTF-8")?;
    let manifest = netsuke::manifest::from_str(&glob_manifest(pattern))
        .context("an unopenable prefix should parse, not fail")?;
    ensure!(
        manifest.targets.is_empty(),
        "expected no targets, got {:?}",
        manifest.targets.len()
    );
    Ok(())
}

/// A parent-relative pattern expands against the working directory.
#[rstest]
fn parent_relative_pattern_expands(temp_dir: tempfile::TempDir) -> Result<()> {
    // Loading the manifest through its own path anchors relative glob
    // patterns to the manifest's workspace root (`sub/`), so `../*.txt`
    // ascends to the temporary directory. This preserves the parent-relative
    // coverage without changing the process working directory.
    let sub = temp_dir.path().join("sub");
    test_support::fs::create_dir(&sub)?;
    test_support::fs::write(temp_dir.path().join("out.txt"), "out")?;
    let manifest_path = sub.join("Netsukefile");
    test_support::fs::write(&manifest_path, glob_manifest("../*.txt"))?;

    let manifest = netsuke::manifest::from_path(&manifest_path)?;
    ensure!(
        target_names(&manifest)? == vec!["../out.txt".to_owned()],
        "expected the parent-relative match"
    );
    Ok(())
}

/// A symbolic link that escapes the literal prefix, and one that dangles,
/// are both dropped from the expansion without failing the manifest.
#[cfg(unix)]
#[rstest]
#[case::escaping("../vendor/escaped.txt")]
#[case::dangling("nowhere.txt")]
fn unresolvable_symlinks_are_skipped(
    temp_dir: tempfile::TempDir,
    #[case] link_target: &str,
) -> Result<()> {
    let src = temp_dir.path().join("src");
    let vendor = temp_dir.path().join("vendor");
    test_support::fs::create_dir(&src)?;
    test_support::fs::create_dir(&vendor)?;
    test_support::fs::write(vendor.join("escaped.txt"), "escaped")?;
    test_support::fs::write(src.join("real.txt"), "real")?;
    test_support::fs::symlink(link_target, src.join("linked.txt"))?;

    let pattern = format!("{}/src/*.txt", temp_dir.path().display());
    let manifest = netsuke::manifest::from_str(&glob_manifest(&pattern))
        .context("an unresolvable link should not fail the manifest")?;
    let names = target_names(&manifest)?;
    ensure!(
        names.iter().any(|n| n.ends_with("/src/real.txt")),
        "the resolvable match should survive: {names:?}"
    );
    ensure!(
        names.iter().all(|n| !n.ends_with("linked.txt")),
        "the unresolvable link should be skipped: {names:?}"
    );
    ensure!(names.len() == 1, "expected exactly one target: {names:?}");
    Ok(())
}
