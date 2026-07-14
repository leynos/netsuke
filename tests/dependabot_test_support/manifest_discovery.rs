//! Regression tests for tracked Cargo manifest discovery.

use super::{TrackedManifests, tracked_cargo_manifest_dirs};
use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use std::{collections::BTreeSet, fs, process::Command};
use tempfile::{TempDir, tempdir};

/// Create a temporary UTF-8 root for an isolated source-tree fixture.
fn temp_root() -> Result<(TempDir, Utf8PathBuf)> {
    let directory = tempdir().context("create temporary source tree")?;
    let root = Utf8PathBuf::from_path_buf(directory.path().to_path_buf())
        .map_err(|path| anyhow::anyhow!("temporary path is not UTF-8: {}", path.display()))?;
    Ok((directory, root))
}

/// Run Git successfully in an isolated source-tree fixture.
fn run_git(root: &Utf8Path, arguments: &[&str]) -> Result<()> {
    let status = Command::new("git")
        .args(["-C", root.as_str()])
        .args(arguments)
        .status()
        .with_context(|| format!("run git {}", arguments.join(" ")))?;
    ensure!(
        status.success(),
        "git {} should succeed",
        arguments.join(" ")
    );
    Ok(())
}

/// Write a manifest-shaped fixture file below an isolated source-tree root.
fn write_fixture(root: &Utf8Path, relative_path: &Utf8Path) -> Result<()> {
    let path = root.join(relative_path);
    let parent = path
        .parent()
        .with_context(|| format!("fixture path {path} should have a parent"))?;
    fs::create_dir_all(parent).with_context(|| format!("create fixture directory {parent}"))?;
    fs::write(
        &path,
        "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\n",
    )
    .with_context(|| format!("write fixture manifest {path}"))?;
    Ok(())
}

#[test]
fn ignores_untracked_cargo_manifest_pollution() -> Result<()> {
    let (_directory, root) = temp_root()?;
    run_git(&root, &["init"])?;
    write_fixture(&root, Utf8Path::new("Cargo.toml"))?;
    run_git(&root, &["add", "Cargo.toml"])?;
    write_fixture(&root, Utf8Path::new("workflow-src/Cargo.toml"))?;

    let manifests = tracked_cargo_manifest_dirs(&root)?;
    ensure!(
        matches!(manifests, TrackedManifests::Dirs(dirs) if dirs == BTreeSet::from([String::from("/")])),
        "untracked Cargo manifest should not appear in tracked manifest directories"
    );
    Ok(())
}

#[test]
fn identifies_source_tree_without_git_checkout() -> Result<()> {
    let (_directory, root) = temp_root()?;
    write_fixture(&root, Utf8Path::new("Cargo.toml"))?;

    ensure!(
        matches!(
            tracked_cargo_manifest_dirs(&root)?,
            TrackedManifests::NotAGitCheckout
        ),
        "source tree without Git metadata should be identified as non-checkout"
    );
    Ok(())
}
