//! Tests for glob pattern normalisation and brace validation.
use super::super::GlobPattern;
#[cfg(unix)]
use super::super::normalize::force_literal_escapes;
use super::super::normalize::normalize_separators;
use super::super::validate::validate_brace_matching;
use crate::localization::{self, keys};
use anyhow::{Context, Result, anyhow, ensure};
use minijinja::ErrorKind;
use rstest::rstest;

/// Helper to assert that a pattern produces a syntax error.
fn assert_syntax_error(pattern: &str, context_msg: &str) -> Result<()> {
    match validate_brace_matching(pattern) {
        Ok(()) => Err(anyhow!("{context_msg}")),
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
            let expected = localization::message(keys::MANIFEST_GLOB_UNMATCHED_BRACE)
                .with_arg("pattern", "foo{")
                .with_arg("character", '{')
                .with_arg("position", 3)
                .to_string();
            ensure!(
                err.to_string().contains(&expected),
                "unexpected error message: {err}"
            );
            Ok(())
        }
        Err(e) => Err(e),
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
