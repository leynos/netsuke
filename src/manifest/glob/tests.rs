//! Tests for glob validation and expansion helpers.
use super::GlobPattern;
use super::validate::validate_brace_matching;
use super::walk::process_glob_entry;
use anyhow::{Context, Result, anyhow, ensure};
use minijinja::ErrorKind;

#[test]
fn validate_brace_matching_accepts_balanced_braces() {
    let pattern = GlobPattern {
        raw: "{foo,bar}".into(),
        normalized: None,
    };
    assert!(validate_brace_matching(&pattern).is_ok());
}

#[test]
fn validate_brace_matching_rejects_unmatched_closing() -> Result<()> {
    let pattern = GlobPattern {
        raw: "foo}".into(),
        normalized: None,
    };
    match validate_brace_matching(&pattern) {
        Ok(()) => Err(anyhow!(
            "validate_brace_matching should fail for pattern {:?}",
            pattern.raw
        )),
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

#[cfg(unix)]
#[test]
fn process_glob_entry_rejects_non_utf8_paths() -> Result<()> {
    use cap_std::{ambient_authority, fs::Dir};
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let root = Dir::open_ambient_dir("/", ambient_authority()).context("open ambient root dir")?;
    let path = std::path::PathBuf::from(OsString::from_vec(b"bad\xFF".to_vec()));
    let pattern = GlobPattern {
        raw: "pattern".into(),
        normalized: None,
    };
    match process_glob_entry(Ok(path), pattern, &root) {
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
