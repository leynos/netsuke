//! Walk the authored manifest's recipes.
//!
//! Most document rules ask the same question — "what shell text did the author
//! write, and where?" — so the walk is shared. Each part carries the source
//! slice rather than only the decoded scalar, because a rule detecting an
//! authored construct such as `$$` or a trailing `&` should scan what the
//! author typed, and because offsets into that slice map straight onto source
//! spans.

use crate::lint::document::{Document, Node, ScalarStyle, Span};

/// Which top-level section an item was declared in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    /// A reusable rule.
    Rules,
    /// An implicitly phony action.
    Actions,
    /// A build target.
    Targets,
}

impl Section {
    /// Name the section as it appears in the manifest.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Rules => "rules",
            Self::Actions => "actions",
            Self::Targets => "targets",
        }
    }

    /// Name one member of the section, for diagnostic prose.
    #[must_use]
    pub const fn member(self) -> &'static str {
        match self {
            Self::Rules => "rule",
            Self::Actions => "action",
            Self::Targets => "target",
        }
    }

    /// Every section that can carry a recipe, in manifest order.
    pub const ALL: [Self; 3] = [Self::Rules, Self::Actions, Self::Targets];
}

/// One declared item of a top-level section.
#[derive(Debug, Clone, Copy)]
pub struct Item<'a> {
    /// The section the item was declared in.
    pub section: Section,
    /// Zero-based position within the section as authored.
    pub index: usize,
    /// The item's mapping node.
    pub node: &'a Node,
}

impl<'a> Item<'a> {
    /// Borrow the item's literal `name` scalar, when it has one.
    #[must_use]
    pub fn name(&self) -> Option<&'a str> {
        self.node.get("name").and_then(Node::as_str)
    }

    /// Name the item for diagnostics, falling back to its position.
    #[must_use]
    pub fn label(&self) -> String {
        self.name().map_or_else(
            || format!("{} {}", self.section.member(), self.index + 1),
            |name| format!("{} `{name}`", self.section.member()),
        )
    }

    /// Report the span of `field`'s key, or of the item when it has no such
    /// field.
    #[must_use]
    pub fn field_span(&self, field: &str) -> Span {
        self.node
            .key_node(field)
            .map_or(self.node.span, |node| node.span)
    }
}

/// Which recipe key a fragment came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecipeKind {
    /// A `command` scalar, or one entry of a `command` list.
    Command,
    /// A `script` block.
    Script,
}

impl RecipeKind {
    /// Name the recipe key as written in the manifest.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Command => "command",
            Self::Script => "script",
        }
    }
}

/// One authored shell fragment together with its source position.
#[derive(Debug, Clone, Copy)]
pub struct RecipePart<'a> {
    /// The item the fragment was declared on.
    pub item: Item<'a>,
    /// Which recipe key the fragment came from.
    pub kind: RecipeKind,
    /// Byte range of the fragment's scalar node.
    pub span: Span,
    /// The manifest source covered by `span`.
    pub source: &'a str,
    /// How the scalar was written.
    pub style: ScalarStyle,
    /// Whether the fragment is one entry of a `command` list.
    pub is_list_entry: bool,
}

impl RecipePart<'_> {
    /// Build a span for the fragment's byte range `start..start + len`,
    /// measured from the start of [`RecipePart::source`].
    #[must_use]
    pub fn sub_span(&self, start: usize, len: usize) -> Span {
        let begin = self.span.start.saturating_add(start).min(self.span.end);
        let end = begin.saturating_add(len).min(self.span.end);
        Span::new(begin, end.max(begin))
    }
}

/// Collect every declared item of every recipe-carrying section.
#[must_use]
pub fn items(doc: &Document) -> Vec<Item<'_>> {
    Section::ALL
        .into_iter()
        .flat_map(|section| {
            doc.section(section.key())
                .into_iter()
                .flat_map(Node::items)
                .enumerate()
                .map(move |(index, node)| Item {
                    section,
                    index,
                    node,
                })
        })
        .collect()
}

/// Collect every authored shell fragment in the manifest.
#[must_use]
pub fn parts(doc: &Document) -> Vec<RecipePart<'_>> {
    items(doc)
        .into_iter()
        .flat_map(|item| item_parts(doc, item))
        .collect()
}

/// Collect the shell fragments declared on one item.
#[must_use]
pub fn item_parts<'a>(doc: &'a Document, item: Item<'a>) -> Vec<RecipePart<'a>> {
    let mut parts = Vec::new();
    if let Some(node) = item.node.get("command") {
        let is_list = node.as_str().is_none();
        for scalar in command_scalars(node) {
            push_part(
                &Fragment {
                    doc,
                    item,
                    kind: RecipeKind::Command,
                    node: scalar,
                    is_list_entry: is_list,
                },
                &mut parts,
            );
        }
    }
    if let Some(node) = item.node.get("script") {
        push_part(
            &Fragment {
                doc,
                item,
                kind: RecipeKind::Script,
                node,
                is_list_entry: false,
            },
            &mut parts,
        );
    }
    parts
}

/// One scalar node being recorded as a recipe fragment.
struct Fragment<'a> {
    /// The document the node belongs to.
    doc: &'a Document,
    /// The item the fragment was declared on.
    item: Item<'a>,
    /// Which recipe key the fragment came from.
    kind: RecipeKind,
    /// The scalar node holding the shell text.
    node: &'a Node,
    /// Whether the node is one entry of a `command` list.
    is_list_entry: bool,
}

/// Record one scalar node as a recipe fragment.
fn push_part<'a>(fragment: &Fragment<'a>, parts: &mut Vec<RecipePart<'a>>) {
    let Some(style) = fragment.node.scalar_style() else {
        return;
    };
    let span = content_span(fragment.doc, fragment.node, style);
    parts.push(RecipePart {
        item: fragment.item,
        kind: fragment.kind,
        span,
        source: fragment.doc.slice(span),
        style,
        is_list_entry: fragment.is_list_entry,
    });
}

/// Collect the scalar nodes a `command` field holds.
///
/// A scalar command is one fragment; a list command is one fragment per entry,
/// because each entry is rendered and lowered independently.
fn command_scalars(node: &Node) -> Vec<&Node> {
    node.as_str()
        .map_or_else(|| node.items().collect(), |_| vec![node])
}

/// Narrow a scalar's span to the shell text it carries.
///
/// A quoted scalar's YAML quotes are not shell quotes, and a block scalar's
/// `|` header is not shell text. Scanning them as though they were would mask
/// the whole recipe as quoted and hide every finding inside it.
fn content_span(doc: &Document, node: &Node, style: ScalarStyle) -> Span {
    let source = doc.slice(node.span);
    let span = match style {
        ScalarStyle::Plain => node.span,
        ScalarStyle::Quoted => strip_quotes(node.span, source),
        ScalarStyle::Block => strip_block_header(node.span, source),
    };
    trim_trailing(doc, span)
}

/// Shrink a span past trailing whitespace.
///
/// A block scalar's reported span runs to the start of whatever follows it, so
/// without this a diagnostic would underline the blank line and the first line
/// of the next declaration.
fn trim_trailing(doc: &Document, span: Span) -> Span {
    let trimmed = doc.slice(span).trim_end();
    Span::new(span.start, span.start.saturating_add(trimmed.len()))
}

/// Drop a quoted scalar's surrounding quote characters.
fn strip_quotes(span: Span, source: &str) -> Span {
    let mut characters = source.chars();
    let opener = characters.next();
    let is_quoted = matches!(opener, Some('\'' | '"'))
        && source.len() >= 2
        && source.ends_with(opener.unwrap_or('"'));
    if is_quoted {
        return Span::new(span.start + 1, span.end - 1);
    }
    span
}

/// Drop a block scalar's header line, keeping its indented body.
fn strip_block_header(span: Span, source: &str) -> Span {
    match source.find('\n') {
        Some(index) if source.starts_with(['|', '>']) => {
            Span::new(span.start + index + 1, span.end)
        }
        _ => span,
    }
}
