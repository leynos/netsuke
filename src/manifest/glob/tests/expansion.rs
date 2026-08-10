//! Tests for the match set [`glob_paths`] returns.
use super::super::walk::{GlobRoot, process_glob_entry};
use super::super::{GlobPattern, glob_paths};
use anyhow::{Context, Result, anyhow, ensure};
use cap_std::{ambient_authority, fs::Dir};
use minijinja::ErrorKind;
use tempfile::tempdir;
use test_support::fs as test_fs;

#[test]
fn glob_paths_filters_directories() -> Result<()> {
    let temp = tempdir()?;
    let dir = temp.path().join("dir");
    test_fs::create_dir(&dir)?;
    let file = temp.path().join("dir").join("file.txt");
    test_fs::write(&file, "data")?;

    let pattern = format!("{}/dir/*", temp.path().display());
    let results = glob_paths(&pattern)?;
    ensure!(
        results.iter().any(|p| p.ends_with("file.txt")),
        "expected file match"
    );
    ensure!(
        results.iter().all(|p| !p.ends_with("/dir")),
        "directories should be filtered out"
    );
    Ok(())
}

#[test]
fn glob_paths_rejects_unmatched_brace() {
    let err = glob_paths("foo{bar").expect_err("brace mismatch should error");
    assert_eq!(err.kind(), ErrorKind::SyntaxError);
}

#[test]
fn glob_paths_rejects_an_invalid_pattern_before_a_missing_prefix() {
    let err = glob_paths("missing/[").expect_err("an invalid pattern should error");
    assert_eq!(err.kind(), ErrorKind::SyntaxError);
}

#[cfg(unix)]
#[test]
fn glob_paths_accepts_escaped_braces_and_matches_files() -> Result<()> {
    let temp = tempdir()?;
    let file = temp.path().join("{file}.txt");
    test_fs::write(&file, "data")?;

    let pattern = format!("{}/\\{{file\\}}.txt", temp.path().display());
    let normalized = GlobPattern::new(&pattern)?;
    ensure!(
        normalized.normalized().contains("[{]file[}]"),
        "unexpected normalized pattern: {}",
        normalized.normalized()
    );
    let results = glob_paths(&pattern)?;
    ensure!(
        results.iter().any(|p| p.ends_with("{file}.txt")),
        "escaped brace pattern should match literal braces"
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn process_glob_entry_rejects_non_utf8_paths() -> Result<()> {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let dir = Dir::open_ambient_dir("/", ambient_authority()).context("open ambient root dir")?;
    let root = GlobRoot::new(dir, camino::Utf8PathBuf::from("/"));
    let path = std::path::PathBuf::from(OsString::from_vec(b"bad\xFF".to_vec()));
    let pattern = GlobPattern::new("pattern")?;
    match process_glob_entry(Ok(path), &pattern, &root) {
        Ok(value) => Err(anyhow!("expected non-UTF-8 error but received {value:?}")),
        Err(err) => {
            ensure!(
                err.kind() == ErrorKind::InvalidOperation,
                "unexpected error kind {kind:?}",
                kind = err.kind()
            );
            Ok(())
        }
    }
}
