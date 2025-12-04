//! Tests for glob validation and expansion helpers.
#[cfg(unix)]
use super::normalize::force_literal_escapes;
use super::normalize::normalize_separators;
use super::validate::validate_brace_matching;
use super::walk::process_glob_entry;
use super::{GlobPattern, glob_paths};
use anyhow::{Context, Result, anyhow, ensure};
use cap_std::{ambient_authority, fs::Dir};
use minijinja::ErrorKind;
use tempfile::tempdir;

#[test]
fn normalize_separators_collapses_mixed_slashes() {
    let normalized = normalize_separators(r"foo\\bar/baz");
    #[cfg(unix)]
    assert_eq!(normalized, "foo//bar/baz");
    #[cfg(not(unix))]
    assert!(normalized.contains(std::path::MAIN_SEPARATOR));
}

#[cfg(unix)]
#[test]
fn force_literal_escapes_preserves_bracket_escapes() {
    let pattern = r"\[foo\]\*\?";
    let forced = force_literal_escapes(pattern);
    assert_eq!(forced, r"[[]foo[]][*][?]");
}

#[test]
fn validate_brace_matching_accepts_balanced_braces() {
    assert!(validate_brace_matching("{foo,bar}").is_ok());
}

#[test]
fn validate_brace_matching_rejects_unmatched_closing() -> Result<()> {
    match validate_brace_matching("foo}") {
        Ok(()) => Err(anyhow!("validate_brace_matching should fail for foo}}")),
        Err(err) => {
            ensure!(
                err.kind() == ErrorKind::SyntaxError,
                "unexpected error kind {kind:?}",
                kind = err.kind()
            );
            Ok(())
        }
    }
}

#[test]
fn glob_paths_filters_directories() -> Result<()> {
    let temp = tempdir()?;
    let dir = temp.path().join("dir");
    std::fs::create_dir(&dir)?;
    let file = temp.path().join("dir").join("file.txt");
    std::fs::write(&file, "data")?;

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

#[cfg(unix)]
#[test]
fn process_glob_entry_rejects_non_utf8_paths() -> Result<()> {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let root = Dir::open_ambient_dir("/", ambient_authority()).context("open ambient root dir")?;
    let path = std::path::PathBuf::from(OsString::from_vec(b"bad\xFF".to_vec()));
    let pattern = GlobPattern {
        raw: "pattern".into(),
        normalized: "pattern".into(),
    };
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
