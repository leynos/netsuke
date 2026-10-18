//! Tests for directive scanning and scoping.

use proptest::prelude::*;
use rstest::rstest;

use super::{Scope, collect};
use crate::lint::document::{Document, Span};

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

/// A directive with no named rule cannot suppress every rule by accident.
#[rstest]
#[case("a: 1  # netsuke-lint: allow -- no rule named\n")]
#[case("a: 1  # netsuke-lint: allow --\n")]
fn an_empty_rule_list_never_names_a_rule(#[case] text: &str) {
    let directive = first!(text);
    assert_eq!(directive.rules, Vec::<String>::new());
    assert!(!directive.names("one"));
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

proptest::proptest! {
    /// A `#` inside a block scalar is never a directive, whatever follows it.
    ///
    /// This is the invariant the span-aware scanner exists to hold: a `script`
    /// block is shell text, and a shell comment in it must not be able to
    /// disable a lint rule. The example cases pin specific shapes; this covers
    /// arbitrary comment bodies inside the block.
    #[test]
    fn a_hash_inside_a_script_block_never_yields_a_directive(
        body in proptest::collection::vec(
            proptest::sample::select(vec![
                "# netsuke-lint: allow background-job -- inside a script",
                "# netsuke-lint-file: allow bashism -- inside a script",
                "echo hi  # netsuke-lint: allow unused-var -- trailing",
                "echo plain",
                "# ordinary comment",
            ]),
            1..4,
        ),
    ) {
        let mut indented = String::new();
        for line in &body {
            indented.push_str("      ");
            indented.push_str(line);
            indented.push('\n');
        }
        let yaml = format!(
            concat!(
                "netsuke_version: \"1.0.0\"\n",
                "targets:\n",
                "  - name: out\n",
                "    script: |\n",
                "{}",
            ),
            indented
        );
        let doc = Document::parse(yaml.clone()).expect("fixture should index");
        proptest::prop_assert_eq!(
            collect(&doc).len(),
            0,
            "a script block's contents are shell text, not directives: {}",
            yaml
        );
    }
}

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

/// The directive text every generated layout uses.
const DIRECTIVE: &str = "# netsuke-lint: allow unused-var -- generated";

/// A generated manifest with the expected coverage of each content line.
struct Layout {
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
        self.text.push('\n');
    }

    /// Append one target, writing the directive into it when it is directed.
    fn target(&mut self, index: usize, target: &GeneratedTarget, directed: Option<Placement>) {
        let covered = directed.is_some();
        let mut name = format!("  - name: t{index}");
        match directed {
            Some(Placement::Above(count)) => {
                (0..count).for_each(|_| self.raw(&format!("  {DIRECTIVE}")));
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
    fn field(&mut self, index: usize, field: Field, covered: bool) {
        match field {
            Field::Command => self.content(&format!("    command: \"echo {index}\""), covered),
            Field::Deps => {
                self.content("    deps:", covered);
                self.content(&format!("      - dep{index}"), covered);
            }
            Field::Phony => self.content("    phony: true", covered),
        }
    }
}

/// Build a manifest whose `directed` target carries the directive.
fn build_layout(targets: &[GeneratedTarget], directed: usize, placement: Placement) -> Layout {
    let mut layout = Layout {
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
    layout.content("  - t0", false);
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
    /// the next declaration.
    #[test]
    fn a_node_directive_covers_exactly_its_declaration(
        targets in proptest::collection::vec(generated_target(), 1..6),
        directed_seed in any::<proptest::sample::Index>(),
        placement in placement(),
    ) {
        let directed = directed_seed.index(targets.len());
        let layout = build_layout(&targets, directed, placement);
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
