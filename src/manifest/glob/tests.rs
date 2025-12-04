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
use rstest::rstest;
use tempfile::tempdir;

/// Helper to assert that a pattern produces a syntax error.
fn assert_syntax_error(pattern: &str, context_msg: &str) -> Result<()> {
    match validate_brace_matching(pattern) {
        Ok(()) => Err(anyhow!("{}", context_msg)),
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
fn normalize_separators_collapses_mixed_slashes() {
    let normalized = normalize_separators(r"foo\\bar/baz");
    #[cfg(unix)]
    assert_eq!(normalized, "foo//bar/baz");
    #[cfg(not(unix))]
    {
        let sep = std::path::MAIN_SEPARATOR;
        let expected = format!("foo{sep}{sep}bar{sep}baz");
        assert_eq!(normalized, expected);
    }
}

#[cfg(unix)]
#[test]
fn force_literal_escapes_preserves_bracket_escapes() {
    let pattern = r"\[foo\]\*\?";
    let forced = force_literal_escapes(pattern);
    assert_eq!(forced, r"[[]foo[]][*][?]");
}

#[cfg(unix)]
#[test]
fn normalize_separators_handles_escaped_tokens() {
    let cases = [
        (r"\[", r"\["),
        (r"\]", r"\]"),
        (r"\{", r"\{"),
        (r"\}", r"\}"),
        (r"\*", r"\*"),
        (r"\*x", r"\*x"),
        (r"\*{", "/*{"),
        (r"\?", r"\?"),
        (r"trailing\\", "trailing/\\"),
    ];
    for (input, expected) in cases {
        let normalized = normalize_separators(input);
        assert_eq!(normalized, expected, "input {input}");
    }
}

#[test]
fn validate_brace_matching_accepts_balanced_braces() {
    assert!(validate_brace_matching("{foo,bar}").is_ok());
}

#[rstest]
#[case("{foo,{bar,baz}}", "nested braces")]
#[case("{a,b}{c,d}", "adjacent braces")]
fn validate_brace_matching_accepts_nested_and_adjacent_braces(
    #[case] pattern: &str,
    #[case] desc: &str,
) -> Result<()> {
    validate_brace_matching(pattern)
        .with_context(|| format!("pattern {pattern} ({desc}) should be valid"))
}

#[rstest]
#[case("[abc{]")]
#[case("[{}]")]
fn validate_brace_matching_ignores_braces_in_character_classes(
    #[case] pattern: &str,
) -> Result<()> {
    validate_brace_matching(pattern)
        .with_context(|| format!("pattern {pattern} should ignore braces"))
}

#[cfg(unix)]
#[test]
fn validate_brace_matching_treats_escaped_braces_as_literals() -> Result<()> {
    validate_brace_matching(r"\{foo\}").context("escaped braces should not affect brace depth")
}

#[cfg(not(unix))]
#[test]
fn validate_brace_matching_counts_escaped_braces() -> Result<()> {
    match validate_brace_matching(r"\{foo") {
        Ok(()) => Err(anyhow!("escaped brace should still count towards depth")),
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

#[cfg(not(unix))]
#[test]
fn validate_brace_matching_counts_escaped_braces() -> Result<()> {
    assert_syntax_error(r"\{foo", "escaped brace should still count towards depth")
}

#[test]
fn validate_brace_matching_rejects_unmatched_closing() -> Result<()> {
    assert_syntax_error("foo}", "validate_brace_matching should fail for foo}")
}

#[test]
fn validate_brace_matching_rejects_unmatched_opening() -> Result<()> {
    match assert_syntax_error("foo{", "validate_brace_matching should fail for foo{") {
        Ok(()) => {
            // Additional message check for opening brace context.
            let err = validate_brace_matching("foo{")
                .expect_err("brace mismatch should produce error after helper pass");
            ensure!(
                err.to_string()
                    .contains("invalid glob pattern 'foo{': unmatched '{' at position 3"),
                "unexpected error message: {err}"
            );
            Ok(())
        }
        Err(e) => Err(e),
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
fn glob_paths_accepts_escaped_braces_and_matches_files() -> Result<()> {
    let temp = tempdir()?;
    let file = temp.path().join("{file}.txt");
    std::fs::write(&file, "data")?;

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

    let root = Dir::open_ambient_dir("/", ambient_authority()).context("open ambient root dir")?;
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

#[test]
fn glob_pattern_new_normalizes_and_validates() -> Result<()> {
    #[cfg(unix)]
    {
        let pattern = GlobPattern::new(r"foo\\bar")?;
        ensure!(
            pattern.raw() == r"foo\\bar",
            "expected raw pattern to remain unchanged"
        );
        ensure!(
            pattern.normalized() == "foo//bar",
            "unexpected normalization"
        );
    }
    #[cfg(not(unix))]
    {
        let pattern = GlobPattern::new("foo\\bar")?;
        let sep = std::path::MAIN_SEPARATOR;
        let expected = format!("foo{sep}bar");
        ensure!(
            pattern.normalized() == expected,
            "unexpected normalization on non-Unix"
        );
    }
    Ok(())
}

#[test]
fn glob_pattern_new_rejects_invalid_braces() {
    let err = GlobPattern::new("foo{").expect_err("invalid brace pattern must fail");
    assert_eq!(err.kind(), ErrorKind::SyntaxError);
}
