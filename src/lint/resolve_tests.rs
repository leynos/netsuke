//! Tests for best-effort source provenance.

use super::{Provenance, entry_span, field_span, node_span};
use crate::ir::BuildGraph;
use crate::lint::document::Document;
use crate::manifest;

/// Parse a fixture into its document and its expanded manifest.
///
/// The graph is built and discarded: it proves the fixture lowers, which is
/// what makes the manifest under test a realistic one.
macro_rules! provenance {
    ($yaml:expr) => {{
        let parsed = manifest::from_str($yaml).expect("fixture should parse");
        BuildGraph::from_manifest(&parsed).expect("fixture should lower");
        let doc = Document::parse($yaml.to_owned()).expect("fixture should index");
        (doc, parsed)
    }};
}

/// Without `foreach`, every section keeps its authored length, so items match
/// positionally.
#[test]
fn positional_correspondence_resolves_every_item() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "rules:\n",
        "  - name: touch\n",
        "    command: \"touch {{ outs }}\"\n",
        "actions:\n",
        "  - name: run\n",
        "    command: \"run\"\n",
        "targets:\n",
        "  - name: a\n",
        "    rule: touch\n",
        "  - name: b\n",
        "    rule: touch\n",
    );
    let (doc, parsed) = provenance!(yaml);
    let found = Provenance::new(&doc, &parsed);
    assert!(found.rule(0).is_some());
    assert!(found.action(0).is_some());
    assert!(found.target(0).is_some());
    assert!(found.target(1).is_some());
    assert!(
        found.target(2).is_none(),
        "an absent item resolves to nothing"
    );
}

/// When `foreach` changes a section's length, a literal name still resolves.
#[test]
fn a_literal_name_resolves_after_expansion() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "vars:\n",
        "  items:\n",
        "    - one\n",
        "    - two\n",
        "targets:\n",
        "  - foreach: items\n",
        "    name: \"{{ item }}\"\n",
        "    command: \"touch {{ outs }}\"\n",
        "  - name: literal\n",
        "    command: \"touch {{ outs }}\"\n",
    );
    let (doc, parsed) = provenance!(yaml);
    let found = Provenance::new(&doc, &parsed);
    let literal = parsed
        .targets
        .iter()
        .position(|target| target.name.to_string_vec() == vec!["literal".to_owned()])
        .expect("the literal target should survive expansion");
    let node = found
        .target(literal)
        .expect("a literal name should resolve");
    assert_eq!(
        node.get("name").and_then(|name| name.as_str()),
        Some("literal")
    );
}

/// A templated name is not literal, so a generated target resolves to nothing
/// rather than to the wrong declaration.
#[test]
fn a_generated_target_resolves_to_nothing() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "vars:\n",
        "  items:\n",
        "    - one\n",
        "    - two\n",
        "targets:\n",
        "  - foreach: items\n",
        "    name: \"{{ item }}\"\n",
        "    command: \"touch {{ outs }}\"\n",
        "  - name: literal\n",
        "    command: \"touch {{ outs }}\"\n",
    );
    let (doc, parsed) = provenance!(yaml);
    let found = Provenance::new(&doc, &parsed);
    let generated = parsed
        .targets
        .iter()
        .position(|target| target.name.to_string_vec() == vec!["one".to_owned()])
        .expect("expansion should produce the generated target");
    assert!(found.target(generated).is_none());
}

#[test]
fn field_and_entry_spans_point_at_the_declaration() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  - name: first\n",
        "    command: \"touch {{ outs }}\"\n",
        "  - name: second\n",
        "    deps:\n",
        "      - first\n",
        "    command: \"touch {{ outs }}\"\n",
    );
    let (doc, parsed) = provenance!(yaml);
    let found = Provenance::new(&doc, &parsed);
    let node = found.target(1);
    let name = field_span(node, "name").expect("the field should resolve");
    assert_eq!(doc.slice(name), "name");
    let entry = entry_span(node, "deps", "first").expect("the entry should resolve");
    assert_eq!(doc.slice(entry), "first");
    assert!(node_span(node).is_some());
    assert!(node_span(None).is_none());
    assert!(field_span(node, "no_such_field").is_none());
}

/// An entry that is not in the list falls back to the list itself, so the
/// reader is still sent to the right declaration.
#[test]
fn a_missing_entry_falls_back_to_its_field() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  - name: only\n",
        "    deps: []\n",
        "    command: \"touch {{ outs }}\"\n",
    );
    let (doc, parsed) = provenance!(yaml);
    let found = Provenance::new(&doc, &parsed);
    assert!(entry_span(found.target(0), "deps", "absent").is_some());
    assert!(entry_span(found.target(0), "no_such_field", "absent").is_none());
}
