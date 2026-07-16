//! Regression tests for tracked Cargo manifest discovery.

use super::{TrackedManifests, tracked_cargo_manifest_dirs};
use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use proptest::{prelude::*, test_runner::TestCaseError};
use rstest::{fixture, rstest};
use std::{collections::BTreeSet, process::Command};
use tempfile::{TempDir, tempdir};

/// Create a temporary UTF-8 root for an isolated source-tree fixture.
#[fixture]
fn temp_root() -> Result<(TempDir, Utf8PathBuf, Dir)> {
    let directory = tempdir().context("create temporary source tree")?;
    let root = Utf8PathBuf::from_path_buf(directory.path().to_path_buf())
        .map_err(|path| anyhow::anyhow!("temporary path is not UTF-8: {}", path.display()))?;
    let root_dir = Dir::open_ambient_dir(&root, ambient_authority())
        .with_context(|| format!("open temporary source tree {root}"))?;
    Ok((directory, root, root_dir))
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
fn write_fixture(root: &Dir, relative_path: &Utf8Path) -> Result<()> {
    if let Some(parent) = relative_path
        .parent()
        .filter(|parent| !parent.as_str().is_empty())
    {
        root.create_dir_all(parent)
            .with_context(|| format!("create fixture directory {parent}"))?;
    }
    root.write(
        relative_path,
        "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\n",
    )
    .with_context(|| format!("write fixture manifest {relative_path}"))?;
    Ok(())
}

#[rstest]
fn ignores_untracked_cargo_manifest_pollution(
    temp_root: Result<(TempDir, Utf8PathBuf, Dir)>,
) -> Result<()> {
    let (_directory, root, root_dir) = temp_root?;
    run_git(&root, &["init"])?;
    write_fixture(&root_dir, Utf8Path::new("Cargo.toml"))?;
    run_git(&root, &["add", "Cargo.toml"])?;
    write_fixture(&root_dir, Utf8Path::new("workflow-src/Cargo.toml"))?;

    let manifests = tracked_cargo_manifest_dirs(&root)?;
    ensure!(
        matches!(manifests, TrackedManifests::Dirs(dirs) if dirs == BTreeSet::from([String::from("/")])),
        "untracked Cargo manifest should not appear in tracked manifest directories"
    );
    Ok(())
}

#[rstest]
fn identifies_source_tree_without_git_checkout(
    temp_root: Result<(TempDir, Utf8PathBuf, Dir)>,
) -> Result<()> {
    let (_directory, root, root_dir) = temp_root?;
    write_fixture(&root_dir, Utf8Path::new("Cargo.toml"))?;

    ensure!(
        matches!(
            tracked_cargo_manifest_dirs(&root)?,
            TrackedManifests::NotAGitCheckout
        ),
        "source tree without Git metadata should be identified as non-checkout"
    );
    Ok(())
}

proptest! {
    /// Tracked manifest discovery is invariant under arbitrary untracked
    /// manifest pollution in disjoint directory namespaces.
    #[test]
    fn generated_layouts_include_only_tracked_manifests(
        tracked_names in proptest::collection::btree_set("[a-z]{1,8}", 0..5),
        untracked_names in proptest::collection::btree_set("[a-z]{1,8}", 1..5),
    ) {
        let (_directory, root, root_dir) = temp_root()
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        run_git(&root, &["init"])
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        write_fixture(&root_dir, Utf8Path::new("Cargo.toml"))
            .map_err(|error| TestCaseError::fail(error.to_string()))?;

        let tracked_paths = tracked_names
            .iter()
            .map(|name| Utf8PathBuf::from(format!("tracked/{name}/Cargo.toml")))
            .collect::<Vec<_>>();
        for path in &tracked_paths {
            write_fixture(&root_dir, path)
                .map_err(|error| TestCaseError::fail(error.to_string()))?;
        }
        for name in &untracked_names {
            let path = Utf8PathBuf::from(format!("untracked/{name}/Cargo.toml"));
            write_fixture(&root_dir, &path)
                .map_err(|error| TestCaseError::fail(error.to_string()))?;
        }

        let mut add_arguments = vec!["add", "Cargo.toml"];
        add_arguments.extend(tracked_paths.iter().map(|path| path.as_str()));
        run_git(&root, &add_arguments)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;

        let actual = match tracked_cargo_manifest_dirs(&root)
            .map_err(|error| TestCaseError::fail(error.to_string()))?
        {
            TrackedManifests::Dirs(directories) => directories,
            TrackedManifests::NotAGitCheckout => {
                return Err(TestCaseError::fail("initialized Git repository was not detected"));
            }
        };
        let mut expected = BTreeSet::from([String::from("/")]);
        expected.extend(tracked_names.iter().map(|name| format!("/tracked/{name}")));

        prop_assert_eq!(actual, expected);
    }
}
