//! Tests for composing isolated child-process `PATH` values.

use anyhow::{Context, Result, ensure};
use rstest::rstest;
use std::ffi::OsStr;
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
    let paths = std::env::split_paths(&composed)
        .filter(|p| !p.as_os_str().is_empty())
        .collect::<Vec<_>>();
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
