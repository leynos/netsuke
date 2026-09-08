//! Tests for quote-aware shell scanning.

use rstest::rstest;

use super::{Mask, find_all, find_words, is_word_bounded, leading_word, segments};

/// An operator outside quotes is shell syntax and is reported.
#[test]
fn a_match_outside_quotes_is_active() {
    assert_eq!(find_all("a && b", "&&").len(), 1);
}

/// An operator inside a shell quote or a Jinja delimiter is text, not
/// syntax, so it is not reported.
#[rstest]
#[case("echo 'a && b'")]
#[case("echo \"a && b\"")]
#[case("echo {{ a && b }}")]
fn a_match_inside_a_quote_or_a_template_is_inert(#[case] text: &str) {
    assert_eq!(
        find_all(text, "&&"),
        Vec::new(),
        "quoted and templated text is not shell syntax"
    );
}

/// A backslash escapes the next character outside single quotes, so an escaped
/// quote does not open or close a string.
#[test]
fn an_escaped_quote_does_not_change_the_state() {
    assert_eq!(find_all("echo \\\" && b", "&&").len(), 1);
}

/// Inside single quotes a backslash is literal, so it cannot escape the
/// closing quote.
#[test]
fn a_single_quote_protects_a_backslash() {
    assert_eq!(find_all("echo 'a\\' && b", "&&").len(), 1);
}

/// A word-bounded needle matches a whole word only, never a substring.
#[rstest]
#[case("make all", "make", 1)]
#[case("makeinfo manual", "make", 0)]
#[case("run/make all", "make", 0)]
#[case("cmake --build", "make", 0)]
fn word_matching_respects_boundaries(
    #[case] text: &str,
    #[case] needle: &str,
    #[case] expected: usize,
) {
    assert_eq!(find_words(text, needle).len(), expected);
}

/// An empty needle matches nothing rather than every position.
#[test]
fn an_empty_needle_matches_nothing() {
    assert_eq!(find_all("anything", ""), Vec::new());
}

/// A match at the start or end of the text is still word-bounded.
#[test]
fn word_boundaries_hold_at_the_ends_of_the_text() {
    assert!(is_word_bounded("make", 0, 4));
    assert!(!is_word_bounded("makes", 0, 4));
    assert!(!is_word_bounded("remake", 2, 4));
}

/// Segments split on shell-active separators and on nothing else.
#[rstest]
#[case("a && b", vec!["a ", " b"])]
#[case("a || b", vec!["a ", " b"])]
#[case("a | b", vec!["a ", " b"])]
#[case("a; b", vec!["a", " b"])]
#[case("a\nb", vec!["a", "b"])]
#[case("echo 'a; b'", vec!["echo 'a; b'"])]
fn segments_split_on_shell_active_separators(#[case] text: &str, #[case] expected: Vec<&str>) {
    let found: Vec<&str> = segments(text).into_iter().map(|(_, part)| part).collect();
    assert_eq!(found, expected);
}

/// Each segment's reported offset indexes back to that segment.
#[test]
fn segment_offsets_locate_each_part() {
    let text = "one && two";
    let found = segments(text);
    for (offset, part) in found {
        assert_eq!(text.get(offset..offset + part.len()), Some(part));
    }
}

/// The leading word skips indentation and any leading assignment.
#[rstest]
#[case("  make all", Some("make"))]
#[case("VAR=1 make all", Some("make"))]
#[case("   ", None)]
fn the_leading_word_skips_indentation_and_assignments(
    #[case] segment: &str,
    #[case] expected: Option<&str>,
) {
    assert_eq!(leading_word(segment).map(|(_, word)| word), expected);
}

/// The leading word's offset indexes to the word itself.
#[test]
fn the_leading_word_reports_its_offset() {
    let segment = "  VAR=1 make all";
    let (offset, word) = leading_word(segment).expect("the segment names a command");
    assert_eq!(segment.get(offset..offset + word.len()), Some(word));
}

/// A shell comment is prose the shell never runs, so constructs inside one
/// must not be reported.
#[rstest]
#[case("# source the environment", "source")]
#[case("cc -c a.c  # use function here", "function")]
#[case("printf x\n# local note", "local")]
fn a_construct_inside_a_shell_comment_is_inert(#[case] text: &str, #[case] needle: &str) {
    assert_eq!(find_words(text, needle), Vec::new());
}

/// A `#` inside a word is ordinary text, and a comment ends at the newline.
#[rstest]
#[case("cc -o a#b source ./env.sh", "source", 1)]
#[case("echo '# source'", "source", 0)]
#[case("# note\nsource ./env.sh", "source", 1)]
fn comment_detection_respects_word_and_line_boundaries(
    #[case] text: &str,
    #[case] needle: &str,
    #[case] expected: usize,
) {
    assert_eq!(find_words(text, needle).len(), expected);
}

/// The leading word is located by position, not by searching for its text.
///
/// `CC=gcc gcc -c a.c` repeats `gcc` inside the assignment, so an offset
/// recovered with `find` would point at the assignment's value instead of the
/// command.
#[test]
fn the_leading_word_offset_skips_a_repeated_assignment_value() {
    let segment = "CC=gcc gcc -c a.c";
    let (offset, word) = leading_word(segment).expect("the segment names a command");
    assert_eq!(word, "gcc");
    assert_eq!(offset, 7, "the offset should point past the assignment");
    assert_eq!(segment.get(offset..offset + word.len()), Some("gcc"));
}

/// An offset past the scanned text is inactive rather than panicking.
#[test]
fn a_mask_reports_nothing_active_past_the_end() {
    assert!(!Mask::new("abc").is_active(99));
}
