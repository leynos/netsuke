//! Property tests for node-scoped suppression coverage.
//!
//! The example tests in `tests.rs` pin the shapes a reader thinks of. These
//! generate manifests whose every content line has a coverage expectation
//! known by construction, so the oracle is how the text was built rather than
//! a second copy of the indentation rule the resolver implements.

use proptest::prelude::*;

use super::{Scope, collect};
use crate::lint::document::{Document, Span};

/// A nested field a generated target may declare.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    /// A one-line `command`.
    Command,
    /// A two-line `deps` list.
    Deps,
    /// A one-line `phony` flag.
    Phony,
}

/// One generated target: the nested fields it declares, each at most once,
/// and whether a blank line separates it from the next declaration.
#[derive(Debug, Clone)]
struct GeneratedTarget {
    /// The fields the target declares, in declaration order.
    fields: Vec<Field>,
    /// Whether a blank line follows the target.
    blank_after: bool,
}

/// Where the directive under test is written.
#[derive(Debug, Clone, Copy)]
enum Placement {
    /// A run of this many directive lines above the target's declaration.
    Above(usize),
    /// One directive trailing the target's declaration line.
    Trailing,
}

/// How a generated manifest is laid out, applied to every line it emits.
#[derive(Debug, Clone, Copy)]
struct Style {
    /// Spaces before each sequence item's `-`, under `targets` and `defaults`.
    item_indent: usize,
    /// Extra spaces before a `deps` entry beyond its key's own indentation.
    list_indent: usize,
    /// The line terminator, `\n` or `\r\n`.
    newline: &'static str,
}

/// The directive text every generated layout uses.
const DIRECTIVE: &str = "# netsuke-lint: allow unused-var -- generated";

/// A generated manifest with the expected coverage of each content line.
struct Layout {
    /// The layout every line follows.
    style: Style,
    /// The manifest text.
    text: String,
    /// The offset of each content line's first non-space byte, paired with
    /// whether the directive must cover it.
    probes: Vec<(usize, bool)>,
    /// How many directives the text carries.
    directives: usize,
}

impl Layout {
    /// Append a content line whose coverage is known by construction.
    fn content(&mut self, line: &str, covered: bool) {
        let indent = line.len() - line.trim_start().len();
        self.probes.push((self.text.len() + indent, covered));
        self.raw(line);
    }

    /// Append a comment or blank line, which no finding can start on.
    fn raw(&mut self, line: &str) {
        self.text.push_str(line);
        self.text.push_str(self.style.newline);
    }

    /// Indent `line` by the item indentation plus `extra` columns.
    fn indented(&self, extra: usize, line: &str) -> String {
        format!(
            "{:width$}{line}",
            "",
            width = self.style.item_indent + extra
        )
    }

    /// Append one target, writing the directive into it when it is directed.
    fn target(&mut self, index: usize, target: &GeneratedTarget, directed: Option<Placement>) {
        let covered = directed.is_some();
        let mut name = self.indented(0, &format!("- name: t{index}"));
        match directed {
            Some(Placement::Above(count)) => {
                let directive = self.indented(0, DIRECTIVE);
                (0..count).for_each(|_| self.raw(&directive));
                self.directives = count;
            }
            Some(Placement::Trailing) => {
                name = format!("{name}  {DIRECTIVE}");
                self.directives = 1;
            }
            None => {}
        }
        self.content(&name, covered);
        for field in &target.fields {
            self.field(index, *field, covered);
        }
        if target.blank_after {
            self.raw("");
        }
    }

    /// Append one nested field of target `index`.
    ///
    /// A key sits two columns past its item's `-`, and a `deps` entry a
    /// further `list_indent` columns past its key.
    fn field(&mut self, index: usize, field: Field, covered: bool) {
        let entry = 2 + self.style.list_indent;
        match field {
            Field::Command => {
                let line = self.indented(2, &format!("command: \"echo {index}\""));
                self.content(&line, covered);
            }
            Field::Deps => {
                let key = self.indented(2, "deps:");
                let item = self.indented(entry, &format!("- dep{index}"));
                self.content(&key, covered);
                self.content(&item, covered);
            }
            Field::Phony => {
                let line = self.indented(2, "phony: true");
                self.content(&line, covered);
            }
        }
    }
}

/// Build a manifest whose `directed` target carries the directive.
fn build_layout(
    style: Style,
    targets: &[GeneratedTarget],
    directed: usize,
    placement: Placement,
) -> Layout {
    let mut layout = Layout {
        style,
        text: String::new(),
        probes: Vec::new(),
        directives: 0,
    };
    layout.content("netsuke_version: \"1.0.0\"", false);
    layout.content("targets:", false);
    for (index, target) in targets.iter().enumerate() {
        layout.target(index, target, (index == directed).then_some(placement));
    }
    layout.content("defaults:", false);
    let default = layout.indented(0, "- t0");
    layout.content(&default, false);
    layout
}

/// Generate one target's shape.
fn generated_target() -> impl Strategy<Value = GeneratedTarget> {
    (
        proptest::sample::subsequence(vec![Field::Command, Field::Deps, Field::Phony], 0..=3),
        any::<bool>(),
    )
        .prop_map(|(fields, blank_after)| GeneratedTarget {
            fields,
            blank_after,
        })
}

/// Generate a layout style: indentation widths and the line terminator.
fn style() -> impl Strategy<Value = Style> {
    (
        0_usize..=4,
        0_usize..=2,
        prop_oneof![Just("\n"), Just("\r\n")],
    )
        .prop_map(|(item_indent, list_indent, newline)| Style {
            item_indent,
            list_indent,
            newline,
        })
}

/// Generate where the directive is written.
fn placement() -> impl Strategy<Value = Placement> {
    prop_oneof![
        (1_usize..4).prop_map(Placement::Above),
        Just(Placement::Trailing)
    ]
}

proptest::proptest! {
    /// A node-scoped directive covers exactly the declaration it governs.
    ///
    /// The expectation comes from how the manifest was built, not from the
    /// resolver: every line of the directed target, nested fields included,
    /// must be covered, and no line of a sibling target or of the surrounding
    /// top-level keys may be. A run of directives above one declaration must
    /// all resolve to it, and a blank separator must not let a block reach
    /// the next declaration. This holds for any item and list indentation
    /// and for LF or CRLF line endings, whose probe offsets are taken from the
    /// emitted bytes.
    #[test]
    fn a_node_directive_covers_exactly_its_declaration(
        targets in proptest::collection::vec(generated_target(), 1..6),
        directed_seed in any::<proptest::sample::Index>(),
        placement in placement(),
        style in style(),
    ) {
        let directed = directed_seed.index(targets.len());
        let layout = build_layout(style, &targets, directed, placement);
        let doc = Document::parse(layout.text.clone()).expect("generated layout should index");
        let found = collect(&doc);
        prop_assert_eq!(found.len(), layout.directives, "directives in {}", layout.text);
        for directive in &found {
            prop_assert!(
                matches!(directive.scope, Scope::Node(_)),
                "a node directive should resolve to a node in {}",
                layout.text
            );
            for &(offset, expected) in &layout.probes {
                prop_assert_eq!(
                    directive.covers(Some(Span::new(offset, offset + 1))),
                    expected,
                    "offset {} in {}",
                    offset,
                    layout.text
                );
            }
        }
    }
}
