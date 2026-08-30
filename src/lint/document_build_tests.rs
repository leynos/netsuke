//! Tests for the spanned document builder.

use rstest::rstest;

use crate::lint::document::{Document, NodeKind, ScalarStyle};

/// Parse `text`, failing the calling test when the fixture will not index.
macro_rules! document {
    ($text:expr) => {
        Document::parse($text.to_owned()).expect("source should index")
    };
}

#[test]
fn scalar_spans_cover_the_authored_text() {
    let text =
        "netsuke_version: \"1.0.0\"\ntargets:\n  - name: out.txt\n    command: \"echo hi\"\n";
    let doc = document!(text);
    let target = doc
        .section("targets")
        .and_then(|node| node.items().next())
        .expect("a target should be indexed");
    let command = target.get("command").expect("command should be indexed");
    assert_eq!(doc.slice(command.span), "\"echo hi\"");
    assert_eq!(command.as_str(), Some("echo hi"));
    assert_eq!(command.scalar_style(), Some(ScalarStyle::Quoted));
}

#[test]
fn spans_are_byte_offsets_even_with_multibyte_text() {
    let text = "netsuke_version: \"1.0.0\"\nvars:\n  gruß: \"schön\"\ntargets:\n  - name: out\n    command: \"echo ✓\"\n";
    let doc = document!(text);
    let command = doc
        .section("targets")
        .and_then(|node| node.items().next())
        .and_then(|node| node.get("command"))
        .expect("command should be indexed");
    assert_eq!(doc.slice(command.span), "\"echo ✓\"");
    let greeting = doc
        .section("vars")
        .and_then(|node| node.get("gruß"))
        .expect("var should be indexed");
    assert_eq!(doc.slice(greeting.span), "\"schön\"");
}

#[test]
fn block_scalars_are_indexed_as_block_style() {
    let text = "netsuke_version: \"1.0.0\"\ntargets:\n  - name: out\n    script: |\n      echo one\n      echo two\n";
    let doc = document!(text);
    let script = doc
        .section("targets")
        .and_then(|node| node.items().next())
        .and_then(|node| node.get("script"))
        .expect("script should be indexed");
    assert_eq!(script.scalar_style(), Some(ScalarStyle::Block));
    assert!(
        doc.slice(script.span).contains("echo one"),
        "block span should cover its content, got {:?}",
        doc.slice(script.span)
    );
    assert!(
        !doc.slice(script.span).contains("name:"),
        "block span should stop at its own body, got {:?}",
        doc.slice(script.span)
    );
    assert_eq!(script.as_str(), Some("echo one\necho two\n"));
}

#[test]
fn mappings_preserve_authored_order_and_key_spans() {
    let text = "netsuke_version: \"1.0.0\"\ntargets:\n  - name: out\n    command: \"echo\"\n";
    let doc = document!(text);
    let target = doc
        .section("targets")
        .and_then(|node| node.items().next())
        .expect("a target should be indexed");
    let entries = target.as_mapping().expect("target should be a mapping");
    let keys: Vec<Option<&str>> = entries.iter().map(|entry| entry.key.as_str()).collect();
    assert_eq!(keys, vec![Some("name"), Some("command")]);
    let key = target.key_node("command").expect("key should be indexed");
    assert_eq!(doc.slice(key.span), "command");
}

#[test]
fn sequences_index_every_entry() {
    let text = "netsuke_version: \"1.0.0\"\ntargets:\n  - name: out\n    command:\n      - \"one\"\n      - \"two\"\n";
    let doc = document!(text);
    let command = doc
        .section("targets")
        .and_then(|node| node.items().next())
        .and_then(|node| node.get("command"))
        .expect("command should be indexed");
    let entries: Vec<Option<&str>> = command.items().map(|item| item.as_str()).collect();
    assert_eq!(entries, vec![Some("one"), Some("two")]);
    assert!(matches!(command.kind, NodeKind::Sequence(_)));
}

#[test]
fn aliases_resolve_to_the_anchored_contents_at_their_own_span() {
    let text = "netsuke_version: \"1.0.0\"\nvars:\n  base: &base \"echo\"\n  copy: *base\ntargets:\n  - name: out\n    command: \"echo\"\n";
    let doc = document!(text);
    let vars = doc.section("vars").expect("vars should be indexed");
    let copy = vars.get("copy").expect("alias should be indexed");
    assert_eq!(copy.as_str(), Some("echo"));
    assert_eq!(doc.slice(copy.span), "*base");
}

#[rstest]
#[case("targets:\n  - name: [\n", 3)]
#[case("targets: [1, 2\n", 2)]
#[case("a: 1\n  b: 2\n", 2)]
fn malformed_sources_report_where_scanning_stopped(#[case] text: &str, #[case] line: usize) {
    let failure = Document::parse(text.to_owned()).expect_err("source should not index");
    assert_eq!(failure.line, line, "{}", failure.message);
    assert_ne!(
        failure.message, "",
        "the scanner should explain the failure"
    );
}

#[test]
fn line_lookup_maps_offsets_to_one_based_lines() {
    let doc = document!("netsuke_version: \"1.0.0\"\ntargets: []\n");
    let targets = doc.section("targets").expect("targets should be indexed");
    assert_eq!(doc.lines().line_of(targets.span.start), 2);
    assert_eq!(doc.lines().line_count(), 3);
}
