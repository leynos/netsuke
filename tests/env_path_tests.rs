//! Tests for composing isolated child-process `PATH` values.

use anyhow::{Context, Result, ensure};
use proptest::prelude::*;
use rstest::rstest;
use std::{ffi::OsStr, path::PathBuf};
use test_support::env::prepend_path_value;

#[rstest]
fn prepend_dir_to_path_preserves_existing_entries() -> Result<()> {
    let original = std::env::join_paths(["one", "two"])?;
    let dir = tempfile::tempdir().context("create temp dir")?;
    let composed = prepend_path_value(Some(&original), dir.path())?;
    let mut split_paths = std::env::split_paths(&composed);
    let first = split_paths
        .next()
        .context("PATH should contain at least one entry after prepend")?;
    ensure!(
        first == dir.path(),
        "expected {} to be first PATH entry, got {}",
        dir.path().display(),
        first.display()
    );
    let remaining = split_paths.collect::<Vec<_>>();
    ensure!(
        remaining == ["one", "two"].map(std::path::PathBuf::from),
        "existing PATH entries should retain their order"
    );
    Ok(())
}

#[rstest]
fn prepend_dir_to_path_handles_empty_path() -> Result<()> {
    let dir = tempfile::tempdir().context("create temp dir")?;
    let composed = prepend_path_value(Some(OsStr::new("")), dir.path())?;
    let paths = std::env::split_paths(&composed).collect::<Vec<_>>();
    ensure!(
        paths == vec![dir.path().to_path_buf()],
        "expected PATH to contain only {}; got {paths:?}",
        dir.path().display()
    );
    Ok(())
}

#[rstest]
fn prepend_dir_to_path_handles_missing_path() -> Result<()> {
    let dir = tempfile::tempdir().context("create temp dir")?;
    let composed = prepend_path_value(None, dir.path())?;
    let paths: Vec<_> = std::env::split_paths(&composed).collect();
    ensure!(
        paths == vec![dir.path().to_path_buf()],
        "expected PATH to contain only {}; got {paths:?}",
        dir.path().display()
    );
    Ok(())
}

proptest! {
    #[test]
    fn prepend_dir_to_path_preserves_every_generated_entry(
        entries in prop::collection::vec("[A-Za-z0-9._-]{1,8}", 0..16),
    ) {
        let original = std::env::join_paths(&entries)
            .expect("generated PATH entries should be joinable");
        let dir = tempfile::tempdir().expect("create property-test temp dir");

        let composed = prepend_path_value(Some(&original), dir.path())
            .expect("prepend generated PATH entries");
        let actual = std::env::split_paths(&composed).collect::<Vec<_>>();
        let expected = std::iter::once(dir.path().to_path_buf())
            .chain(entries.into_iter().map(PathBuf::from))
            .collect::<Vec<_>>();

        prop_assert_eq!(actual, expected);
    }
}
