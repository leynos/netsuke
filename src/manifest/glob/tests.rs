//! Tests for brace validation and glob expansion helpers.

use super::*;
use super::normalize::{force_literal_escapes, normalize_separators, process_escape_sequence};
use super::validate::validate_brace_matching;
use super::walk::process_glob_entry;
use anyhow::{Context, Result, anyhow, ensure};
use cap_std::{ambient_authority, fs::Dir};
use minijinja::ErrorKind;
use rstest::rstest;
use tempfile::tempdir;

fn pattern(raw: &str) -> GlobPattern {
    GlobPattern {
        raw: raw.to_owned(),
        normalized: None,
    }
}

fn build_validator() -> BraceValidator {
    BraceValidator::new()
}

#[test]
fn char_context_retains_flags() {
    let ctx = CharContext {
        ch: '{',
        position: 3,
        in_class: true,
        escaped: true,
    };

    assert_eq!(ctx.ch, '{');
    assert_eq!(ctx.position, 3);
    assert!(ctx.in_class);
    assert!(ctx.escaped);
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
    let validator = build_validator();

    assert!(validator.state.escape_active);
}

#[cfg(not(unix))]
#[test]
fn brace_validator_disables_escape_on_windows() {
    let validator = build_validator();

    assert!(!validator.state.escape_active);
}

#[cfg(unix)]
#[test]
fn handle_escape_sequence_sets_flag_on_unix() {
    let mut validator = build_validator();
    let ctx = CharContext {
        ch: '\\',
        position: 0,
        in_class: false,
        escaped: false,
    };

    let result = validator.handle_escape_sequence(&ctx);

    assert!(result.is_some());
    assert!(validator.escaped);
}

#[cfg(not(unix))]
#[test]
fn handle_escape_sequence_ignored_on_windows() {
    let mut validator = build_validator();
    let ctx = CharContext {
        ch: '\\',
        position: 0,
        in_class: false,
        escaped: false,
    };

    let result = validator.handle_escape_sequence(&ctx);

    assert!(result.is_none());
    assert!(!validator.escaped);
}

#[test]
fn handle_escape_sequence_resets_after_escaped_char() {
    let mut validator = build_validator();
    validator.escaped = true;
    let ctx = CharContext {
        ch: '{',
        position: 1,
        in_class: false,
        escaped: true,
    };

    let result = validator
        .handle_escape_sequence(&ctx)
        .expect("expected escape handling result");

    assert!(result.is_ok());
    assert!(!validator.escaped);
}

#[test]
fn character_class_state_transitions() {
    let mut validator = build_validator();
    let open = CharContext {
        ch: '[',
        position: 0,
        in_class: false,
        escaped: false,
    };
    validator.handle_character_class(&open);
    assert!(validator.state.in_class);

    let close = CharContext {
        ch: ']',
        position: 1,
        in_class: true,
        escaped: false,
    };
    validator.handle_character_class(&close);
    assert!(!validator.state.in_class);
}

#[test]
fn braces_inside_class_do_not_change_depth() {
    let pattern = pattern("[{a}]");
    let mut validator = build_validator();
    for (pos, ch) in pattern.raw.char_indices() {
        validator
            .process_character(ch, pos, &pattern)
            .expect("processing inside class should succeed");
    }

    assert_eq!(validator.state.depth, 0);
}

#[test]
fn nested_open_braces_update_last_open_position() {
    let pattern = pattern("{{}}");
    let mut validator = build_validator();
    for (pos, ch) in pattern.raw.char_indices() {
        validator
            .process_character(ch, pos, &pattern)
            .expect("processing nested braces should succeed");
    }

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

    validate_brace_matching(&pattern)
        .context("pattern with escaped braces should be valid")
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
