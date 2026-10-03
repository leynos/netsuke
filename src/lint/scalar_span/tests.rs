//! Tests for narrowing scanner-reported scalar spans.

use rstest::rstest;

use super::narrow;
use crate::lint::document::{ScalarStyle, Span};

/// Narrow the whole of `text` under `style` and return the covered slice.
fn narrowed(text: &str, style: ScalarStyle) -> &str {
    let span = narrow(text, Span::new(0, text.len()), style);
    text.get(span.start..span.end).unwrap_or_default()
}

#[rstest]
#[case("\"echo hi\"   # comment", "\"echo hi\"")]
#[case("'echo hi'   trailing", "'echo hi'")]
#[case("\"a \\\" b\"  rest", "\"a \\\" b\"")]
#[case("'it''s'  rest", "'it''s'")]
fn a_quoted_scalar_stops_at_its_closing_quote(#[case] text: &str, #[case] expected: &str) {
    assert_eq!(narrowed(text, ScalarStyle::Quoted), expected);
}

/// An unterminated quote cannot be narrowed, so the scanner's span stands
/// minus its trailing whitespace.
#[test]
fn an_unterminated_quote_keeps_the_reported_span() {
    assert_eq!(narrowed("\"echo hi   ", ScalarStyle::Quoted), "\"echo hi");
}

#[rstest]
#[case("value  # comment", "value")]
#[case("value   ", "value")]
#[case("a#b  # comment", "a#b")]
#[case("value", "value")]
fn a_plain_scalar_stops_before_a_comment(#[case] text: &str, #[case] expected: &str) {
    assert_eq!(narrowed(text, ScalarStyle::Plain), expected);
}

/// A block scalar owns its header line plus every more-indented line. The
/// scanner's span can reach into the next declaration, which would make a
/// diagnostic underline unrelated text.
#[test]
fn a_block_scalar_stops_at_its_indented_body() {
    let text = "    script: |\n      echo one\n      echo two\n\n  - name: next\n";
    let span = narrow(text, Span::new(4, text.len()), ScalarStyle::Block);
    let covered = text.get(span.start..span.end).unwrap_or_default();
    assert!(covered.contains("echo two"), "got {covered:?}");
    assert!(!covered.contains("next"), "got {covered:?}");
}

/// A blank line inside a block body is part of the block, so the span must
/// reach past it to the indented content beyond.
#[test]
fn a_blank_line_inside_a_block_does_not_end_it() {
    let text = "    script: |\n      one\n\n      two\n  - name: next\n";
    let span = narrow(text, Span::new(4, text.len()), ScalarStyle::Block);
    let covered = text.get(span.start..span.end).unwrap_or_default();
    assert!(covered.contains("two"), "got {covered:?}");
    assert!(!covered.contains("next"), "got {covered:?}");
}

#[test]
fn an_impossible_span_is_returned_unchanged() {
    let span = Span::new(90, 100);
    assert_eq!(narrow("short", span, ScalarStyle::Plain), span);
}
