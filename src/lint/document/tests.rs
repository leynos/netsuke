//! Tests for the spanned document's navigation and line table.

use rstest::rstest;

use super::{Document, LineIndex, Span};

#[test]
fn a_span_reports_its_extent_and_containment() {
    let outer = Span::new(4, 12);
    assert_eq!(outer.len(), 8);
    assert!(!outer.is_empty());
    assert!(outer.contains(Span::new(6, 8)));
    assert!(outer.contains(outer));
    assert!(!outer.contains(Span::new(2, 8)));
    assert!(outer.contains_offset(4));
    assert!(!outer.contains_offset(12), "the end is exclusive");
    assert!(Span::new(3, 3).is_empty());
}

#[rstest]
#[case("a\nbb\nccc\n", 0, 1)]
#[case("a\nbb\nccc\n", 2, 2)]
#[case("a\nbb\nccc\n", 5, 3)]
fn the_line_table_maps_offsets_to_one_based_lines(
    #[case] text: &str,
    #[case] offset: usize,
    #[case] expected: usize,
) {
    assert_eq!(LineIndex::new(text).line_of(offset), expected);
}

#[test]
fn a_line_span_excludes_its_terminator() {
    let text = "alpha\nbeta\n";
    let index = LineIndex::new(text);
    let span = index.line_span(1, text);
    assert_eq!(text.get(span.start..span.end), Some("alpha"));
}

/// A CRLF line span excludes both bytes of its terminator.
///
/// Leaving the `\r` inside would put it in every directive scan's line text
/// and in the block spans computed from those lines.
#[test]
fn a_crlf_line_span_excludes_the_whole_terminator() {
    let text = "alpha\r\nbeta\r\n";
    let index = LineIndex::new(text);
    for (line, expected) in [(1, "alpha"), (2, "beta")] {
        let span = index.line_span(line, text);
        assert_eq!(
            text.get(span.start..span.end),
            Some(expected),
            "line {line} should omit its terminator"
        );
    }
}

/// A line number past the end must not panic; the linter asks about lines that
/// a directive scan is walking towards.
#[test]
fn an_out_of_range_line_yields_an_empty_span_at_the_end() {
    let text = "alpha\n";
    let index = LineIndex::new(text);
    let span = index.line_span(99, text);
    assert!(span.is_empty());
    assert_eq!(span.start, text.len());
}

#[test]
fn a_document_navigates_sections_and_slices_spans() {
    let text = "netsuke_version: \"1.0.0\"\ntargets:\n  - name: out\n";
    let doc = Document::parse(text.to_owned()).expect("fixture should index");
    let targets = doc.section("targets").expect("targets should be indexed");
    assert_eq!(targets.items().count(), 1);
    assert!(doc.section("rules").is_none());
    let name = targets
        .items()
        .next()
        .and_then(|item| item.get("name"))
        .expect("name should be indexed");
    assert_eq!(doc.slice(name.span), "out");
    assert_eq!(doc.text(), text);
}

/// A malformed span must yield nothing rather than panicking, because span
/// arithmetic happens on every finding.
#[test]
fn slicing_an_impossible_span_yields_nothing() {
    let doc = Document::parse("a: 1\n".to_owned()).expect("fixture should index");
    assert_eq!(doc.slice(Span::new(400, 500)), "");
}

#[test]
fn accessors_report_nothing_for_the_wrong_node_shape() {
    let doc = Document::parse("a: 1\nb: [x]\n".to_owned()).expect("fixture should index");
    let root = doc.root().expect("the document should have a root");
    let scalar = root.get("a").expect("a should be indexed");
    assert!(scalar.as_sequence().is_none());
    assert!(scalar.as_mapping().is_none());
    assert_eq!(scalar.items().count(), 0);
    let sequence = root.get("b").expect("b should be indexed");
    assert!(sequence.as_str().is_none());
    assert!(sequence.get("x").is_none());
}

#[test]
fn the_innermost_node_covering_an_offset_is_the_smallest_one() {
    let text = "targets:\n  - name: out\n";
    let doc = Document::parse(text.to_owned()).expect("fixture should index");
    let root = doc.root().expect("the document should have a root");
    let offset = text
        .find("out")
        .expect("the fixture should contain the value");
    let found = root
        .innermost_covering(offset)
        .expect("an offset inside the document should resolve");
    assert_eq!(doc.slice(found.span), "out");
}
