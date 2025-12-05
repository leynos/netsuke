//! Tests for brace validation and glob expansion helpers.

use super::normalize::normalize_separators;
#[cfg(unix)]
use super::normalize::{force_literal_escapes, process_escape_sequence};
use super::validate::{BraceValidator, CharContext, validate_brace_matching};
use super::walk::process_glob_entry;
use super::*;
use anyhow::{Context, Result, anyhow, ensure};
use cap_std::{ambient_authority, fs::Dir};
use minijinja::ErrorKind;
use rstest::{fixture, rstest};
use tempfile::tempdir;

fn pattern(raw: &str) -> GlobPattern {
    GlobPattern {
        raw: raw.to_owned(),
        normalized: String::new(),
    }
}

#[fixture]
fn build_validator() -> BraceValidator {
    BraceValidator::new()
}

fn build_char_context(ch: char, position: usize) -> CharContext {
    CharContext { ch, position }
}

fn process_pattern_through_validator(
    pattern: &GlobPattern,
    validator: &mut BraceValidator,
) -> Result<()> {
    for (pos, ch) in pattern.raw.char_indices() {
        validator.process_character(ch, pos, pattern)?;
    }
    Ok(())
}

#[test]
fn char_context_retains_flags() {
    let ctx = build_char_context('{', 3);

    assert_eq!(ctx.ch, '{');
    assert_eq!(ctx.position, 3);
}

#[test]
fn brace_state_defaults_are_zeroed() {
    let validator = build_validator();

    assert_eq!(validator.state.depth, 0);
    assert!(!validator.state.in_class);
    assert!(validator.state.last_open_pos.is_none());
}

#[cfg(unix)]
#[test]
fn brace_validator_enables_escape_on_unix() {
    let mut validator = build_validator();
    assert!(validator.handle_escape_sequence('\\'));
}

#[cfg(not(unix))]
#[test]
fn brace_validator_disables_escape_on_windows() {
    let mut validator = build_validator();
    assert!(!validator.handle_escape_sequence('\\'));
}

#[cfg(unix)]
#[test]
fn handle_escape_sequence_sets_flag_on_unix() {
    let mut validator = build_validator();
    let ctx = build_char_context('\\', 0);

    let result = validator.handle_escape_sequence(ctx.ch);

    assert!(result);
    assert!(validator.escaped);
}

#[cfg(not(unix))]
#[test]
fn handle_escape_sequence_ignored_on_windows() {
    let mut validator = build_validator();
    let ctx = build_char_context('\\', 0);

    let result = validator.handle_escape_sequence(ctx.ch);

    assert!(!result);
    assert!(!validator.escaped);
}

#[test]
fn handle_escape_sequence_resets_after_escaped_char() {
    let mut validator = build_validator();
    validator.escaped = true;
    let ctx = build_char_context('{', 1);

    let result = validator.handle_escape_sequence(ctx.ch);

    assert!(result);
    assert!(!validator.escaped);
}

#[test]
fn character_class_state_transitions() {
    let mut validator = build_validator();
    let open = build_char_context('[', 0);
    validator.handle_character_class(open.ch);
    assert!(validator.state.in_class);

    let close = build_char_context(']', 1);
    validator.handle_character_class(close.ch);
    assert!(!validator.state.in_class);
}

#[test]
fn braces_inside_class_do_not_change_depth() {
    let pattern = pattern("[{a}]");
    let mut validator = build_validator();
    process_pattern_through_validator(&pattern, &mut validator)
        .expect("processing inside class should succeed");

    assert_eq!(validator.state.depth, 0);
}

#[test]
fn nested_open_braces_update_last_open_position() {
    let pattern = pattern("{{}}");
    let mut validator = build_validator();
    process_pattern_through_validator(&pattern, &mut validator)
        .expect("processing nested braces should succeed");

    assert_eq!(validator.state.depth, 0);
    assert_eq!(validator.state.last_open_pos, Some(1));
}

#[test]
fn unmatched_closing_brace_returns_error() {
    let pattern = pattern("foo}");
    let mut validator = build_validator();
    let err = validator
        .process_character('}', 3, &pattern)
        .expect_err("expected unmatched closing brace");

    assert_eq!(err.kind(), ErrorKind::SyntaxError);
    assert!(err.to_string().contains("position 3"));
}

#[test]
fn validate_final_state_reports_last_open_position() {
    let pattern = pattern("{foo");
    let mut validator = build_validator();
    validator.state.depth = 1;
    validator.state.last_open_pos = Some(0);

    let err = validator
        .validate_final_state(&pattern)
        .expect_err("expected unmatched open brace");

    assert_eq!(err.kind(), ErrorKind::SyntaxError);
    assert!(err.to_string().contains("position 0"));
}

#[cfg(unix)]
#[test]
fn escaped_braces_do_not_affect_depth() -> Result<()> {
    let pattern = pattern("\\{foo\\}");

    validate_brace_matching(&pattern).context("escaped braces should be valid")
}

#[cfg(not(unix))]
#[test]
fn escaped_open_brace_is_treated_as_literal_on_windows() {
    let pattern = pattern("\\{");

    let err = validate_brace_matching(&pattern)
        .expect_err("escaped brace should be treated as unmatched on Windows");

    assert_eq!(err.kind(), ErrorKind::SyntaxError);
    assert!(err.to_string().contains("position 1"));
}

#[rstest]
#[case("")]
#[case("plain/text")]
#[case("{foo,bar}")]
#[case("{{a,b},{c,d}}")]
#[case("a{b{c,d}e}f")]
#[case("{a{b,c},d{e,f}}")]
#[case("path/{to,from}/file")]
fn validate_brace_matching_accepts_balanced_patterns(#[case] raw: &str) {
    let pattern = pattern(raw);

    assert!(validate_brace_matching(&pattern).is_ok());
}

#[rstest]
#[case("{foo", '{', 0)]
#[case("foo}", '}', 3)]
#[case("{a{b}", '{', 2)]
#[case("}", '}', 0)]
#[case("{a{b}c}d}", '}', 8)]
fn validate_brace_matching_rejects_unbalanced_patterns(
    #[case] raw: &str,
    #[case] expected_char: char,
    #[case] expected_pos: usize,
) {
    let pattern = pattern(raw);

    let err =
        validate_brace_matching(&pattern).expect_err("expected brace mismatch to surface an error");

    assert_eq!(err.kind(), ErrorKind::SyntaxError);
    let msg = err.to_string();
    assert!(msg.contains(&format!("'{expected_char}'")), "{msg}");
    assert!(msg.contains(&format!("position {expected_pos}")), "{msg}");
}

#[test]
fn brace_characters_inside_class_are_ignored() {
    let pattern = pattern("file[{}].txt");

    assert!(validate_brace_matching(&pattern).is_ok());
}

#[test]
fn unclosed_character_class_does_not_block_validation() {
    let pattern = pattern("file[{");

    assert!(validate_brace_matching(&pattern).is_ok());
}

#[cfg(unix)]
#[rstest]
#[case("\\*", "[*]")]
#[case("\\?", "[?]")]
#[case("\\[", "[[]")]
#[case("\\]", "[]]")]
#[case("\\{", "[{]")]
#[case("\\}", "[}]")]
#[case("\\x", "\\x")]
#[case("\\", "\\")]
fn process_escape_sequence_rewrites_expected_tokens(#[case] raw: &str, #[case] expected: &str) {
    fn rewrite(raw: &str) -> String {
        let mut chars = raw.chars().peekable();
        assert_eq!(chars.next(), Some('\\'));
        let mut out = String::new();
        process_escape_sequence(&mut chars, &mut out);
        for ch in chars {
            out.push(ch);
        }
        out
    }

    assert_eq!(rewrite(raw), expected);
}

#[cfg(unix)]
#[test]
fn normalize_separators_converts_backslash_on_unix() {
    let converted = normalize_separators("dir\\file");

    assert_eq!(converted, "dir/file");
}

#[cfg(unix)]
#[test]
fn normalize_separators_preserves_escape_before_brace() {
    let converted = normalize_separators("\\{");

    assert_eq!(converted, "\\{");
}

#[cfg(not(unix))]
#[test]
fn normalize_separators_rewrites_forward_slashes_on_windows() {
    let converted = normalize_separators("dir/to/file");
    let expected = format!(
        "dir{}to{}file",
        std::path::MAIN_SEPARATOR,
        std::path::MAIN_SEPARATOR
    );

    assert_eq!(converted, expected);
}

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

#[cfg(unix)]
#[test]
fn escaped_braces_inside_pattern_validate() -> Result<()> {
    let pattern = pattern("path/\\{escaped\\}/file");

    validate_brace_matching(&pattern).context("pattern with escaped braces should be valid")
}

#[test]
fn pattern_normalized_field_contains_expected_value() -> Result<()> {
    let mut pattern = pattern("test/pattern");
    pattern.normalized = normalize_separators(&pattern.raw);

    ensure!(
        pattern.normalized == normalize_separators("test/pattern"),
        "normalized value should match separator normalization"
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn normalized_applies_escape_forcing_on_unix() -> Result<()> {
    let mut pattern = pattern("test\\*pattern");
    let normalized = normalize_separators(&pattern.raw);
    pattern.normalized = force_literal_escapes(&normalized);

    ensure!(
        pattern.normalized.contains("[*]"),
        "expected forced escape for wildcard"
    );
    Ok(())
}

#[cfg(not(unix))]
#[test]
fn normalized_converts_separators_on_windows() -> Result<()> {
    let mut pattern = pattern("test/pattern");
    pattern.normalized = normalize_separators(&pattern.raw);

    ensure!(
        pattern.normalized.contains(std::path::MAIN_SEPARATOR),
        "expected native separator in normalized pattern"
    );
    Ok(())
}

#[test]
fn open_root_dir_uses_filesystem_root_for_absolute_pattern() -> Result<()> {
    let mut pattern = pattern("/absolute/path");
    pattern.normalized = normalize_separators(&pattern.raw);

    let dir =
        open_root_dir(&pattern).context("open_root_dir should succeed for absolute pattern")?;

    dir.metadata(".")
        .context("should be able to stat root directory")?;
    Ok(())
}

#[test]
fn open_root_dir_uses_current_dir_for_relative_pattern() -> Result<()> {
    let mut pattern = pattern("relative/path");
    pattern.normalized = normalize_separators(&pattern.raw);

    let dir =
        open_root_dir(&pattern).context("open_root_dir should succeed for relative pattern")?;

    dir.metadata(".")
        .context("should be able to stat current directory")?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn open_root_dir_handles_normalized_unix_pattern() -> Result<()> {
    let temp = tempdir()?;
    let subdir = temp.path().join("subdir");
    std::fs::create_dir(&subdir)?;

    let pattern_str = format!("{}/subdir/*", temp.path().display());
    let mut pattern = pattern(&pattern_str);
    pattern.normalized = normalize_separators(&pattern.raw);
    pattern.normalized = force_literal_escapes(&pattern.normalized);

    open_root_dir(&pattern).context("open_root_dir should work with normalized Unix pattern")?;
    Ok(())
}

#[test]
fn glob_paths_normalizes_pattern_before_expansion() -> Result<()> {
    let temp = tempdir()?;
    let file = temp.path().join("test.txt");
    std::fs::write(&file, "content")?;

    let pattern = format!("{}/test.txt", temp.path().display());
    let results = glob_paths(&pattern).context("glob_paths should normalize and expand pattern")?;

    ensure!(results.len() == 1, "expected exactly one match");
    let first = results
        .first()
        .context("expected at least one match from glob_paths")?;
    ensure!(first.ends_with("test.txt"), "expected test.txt in results");
    Ok(())
}

#[cfg(unix)]
#[test]
fn process_glob_entry_rejects_non_utf8_paths() -> Result<()> {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let root = Dir::open_ambient_dir("/", ambient_authority()).context("open ambient root dir")?;
    let path = std::path::PathBuf::from(OsString::from_vec(b"bad\xFF".to_vec()));
    let pattern = pattern("pattern");
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
