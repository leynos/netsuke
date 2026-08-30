//! Tests for directive scanning and scoping.

use rstest::rstest;

use super::{Scope, collect};
use crate::lint::document::Document;

/// Parse `text` and collect its directives.
macro_rules! directives {
    ($text:expr) => {
        collect(&Document::parse($text.to_owned()).expect("fixture should index"))
    };
}

/// Collect a fixture's directives and take the first one.
macro_rules! first {
    ($text:expr) => {
        directives!($text)
            .into_iter()
            .next()
            .expect("the fixture should carry a directive")
    };
}

#[test]
fn a_directive_records_its_rules_and_reason() {
    const TEXT: &str = "a: 1  # netsuke-lint: allow one, two -- because\n";
    assert_eq!(directives!(TEXT).len(), 1);
    let directive = first!(TEXT);
    assert_eq!(directive.rules, vec!["one".to_owned(), "two".to_owned()]);
    assert_eq!(directive.reason.as_deref(), Some("because"));
}

#[rstest]
#[case("a: 1  # netsuke-lint: allow one\n", None)]
#[case("a: 1  # netsuke-lint: allow one --\n", None)]
#[case("a: 1  # netsuke-lint: allow one --   \n", None)]
#[case("a: 1  # netsuke-lint: allow one -- why\n", Some("why"))]
fn a_reason_is_recorded_only_when_stated(#[case] text: &str, #[case] expected: Option<&str>) {
    assert_eq!(first!(text).reason.as_deref(), expected);
}

/// A `#` inside a scalar is content, not a comment. Without this the shell
/// comments in a `script:` block would disable rules.
#[rstest]
#[case("a: \"# netsuke-lint: allow one -- quoted\"\n")]
#[case("a: |\n  # netsuke-lint: allow one -- shell comment\n  echo hi\n")]
#[case("a: 'x#netsuke-lint: allow one -- joined'\n")]
fn a_hash_inside_a_scalar_is_not_a_directive(#[case] text: &str) {
    assert_eq!(
        directives!(text).len(),
        0,
        "a scalar's contents should not be scanned for directives"
    );
}

#[test]
fn an_ordinary_comment_is_not_a_directive() {
    assert_eq!(directives!("# just a note\na: 1\n").len(), 0);
}

/// A directive above a list item governs the whole item, including every line
/// indented beneath it.
#[test]
fn a_leading_directive_scopes_to_the_block_beneath_it() {
    let text = concat!(
        "targets:\n",
        "  # netsuke-lint: allow one -- because\n",
        "  - name: first\n",
        "    command: \"a\"\n",
        "  - name: second\n",
    );
    let doc = Document::parse(text.to_owned()).expect("fixture should index");
    let directive = first!(text);
    let Scope::Node(span) = directive.scope else {
        panic!(
            "the directive should scope to a block, got {:?}",
            directive.scope
        );
    };
    let covered = doc.slice(span);
    assert!(covered.contains("first"), "the block should cover its item");
    assert!(
        covered.contains("command"),
        "the block should cover the item's indented lines"
    );
    assert!(
        !covered.contains("second"),
        "the block should stop before the next item, got {covered:?}"
    );
}

/// A run of comments above one declaration all govern that declaration.
#[test]
fn consecutive_directives_share_one_block() {
    let text = concat!(
        "targets:\n",
        "  # netsuke-lint: allow one -- first\n",
        "  # netsuke-lint: allow two -- second\n",
        "  - name: first\n",
        "    command: \"a\"\n",
    );
    let scopes: Vec<Scope> = directives!(text)
        .iter()
        .map(|directive| directive.scope)
        .collect();
    assert_eq!(scopes.len(), 2);
    assert_eq!(
        scopes.first(),
        scopes.last(),
        "both directives should govern the same declaration"
    );
}

#[test]
fn a_trailing_directive_scopes_to_its_own_line() {
    let text = concat!(
        "targets:\n",
        "  - name: first\n",
        "    command: \"a\"  # netsuke-lint: allow one -- because\n",
        "  - name: second\n",
    );
    let doc = Document::parse(text.to_owned()).expect("fixture should index");
    let directive = first!(text);
    let Scope::Node(span) = directive.scope else {
        panic!("the directive should scope to a block");
    };
    let covered = doc.slice(span);
    assert!(covered.contains("command"));
    assert!(!covered.contains("second"));
}

#[test]
fn a_file_directive_covers_everything() {
    let directive = first!("# netsuke-lint-file: allow one -- everywhere\na: 1\n");
    assert_eq!(directive.scope, Scope::File);
    assert!(
        directive.covers(None),
        "a file directive covers spanless findings"
    );
}

#[test]
fn a_directive_names_only_the_rules_it_lists() {
    let directive = first!("a: 1  # netsuke-lint: allow one -- because\n");
    assert!(directive.names("one"));
    assert!(!directive.names("two"));
}
