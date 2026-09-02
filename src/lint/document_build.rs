//! Build a spanned document from the YAML event stream.
//!
//! The scanner reports positions as character indices, so this module keeps a
//! character-to-byte table and converts every marker before it becomes a
//! [`Span`]. Anchors and aliases are resolved by substituting the anchored
//! node's contents while keeping the alias's own span, which is what a lint
//! diagnostic should point at.

use std::collections::HashMap;

use saphyr_parser::{Event, Parser, ScalarStyle as YamlStyle, Span as YamlSpan};

use super::document::{Entry, Node, NodeKind, ScalarStyle, Span};
use super::scalar_span;

/// A YAML scanner failure encountered while indexing the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseFailure {
    /// The scanner's message.
    pub message: String,
    /// One-based line the scanner stopped at.
    pub line: usize,
}

/// Parse `text` into a spanned tree, or report where scanning failed.
///
/// # Errors
///
/// Returns a [`ParseFailure`] when the source is not well-formed YAML.
pub fn parse_document(text: &str) -> Result<Option<Node>, ParseFailure> {
    let offsets = ByteOffsets::new(text);
    let mut builder = Builder::new(&offsets, text);
    for scanned in Parser::new_from_str(text) {
        let (event, span) = scanned.map_err(|error| ParseFailure {
            message: error.to_string(),
            line: error.marker().line(),
        })?;
        if builder.accept(&event, span) {
            break;
        }
    }
    Ok(builder.finish())
}

/// Maps the scanner's character indices onto byte offsets.
///
/// An all-ASCII manifest needs no table, which is the common case; anything
/// else pays one `usize` per character while the document is being built.
enum ByteOffsets {
    /// Character indices equal byte offsets.
    Ascii {
        /// Length of the source in bytes.
        len: usize,
    },
    /// Byte offset of each character, with a trailing end-of-source sentinel.
    Table(Vec<usize>),
}

impl ByteOffsets {
    /// Build the mapping for `text`.
    fn new(text: &str) -> Self {
        if text.is_ascii() {
            return Self::Ascii { len: text.len() };
        }
        let mut table: Vec<usize> = text.char_indices().map(|(offset, _)| offset).collect();
        table.push(text.len());
        Self::Table(table)
    }

    /// Convert a character index into a byte offset, clamping past the end.
    fn byte_of(&self, chars: usize) -> usize {
        match self {
            Self::Ascii { len } => chars.min(*len),
            Self::Table(table) => table
                .get(chars)
                .copied()
                .unwrap_or_else(|| table.last().copied().unwrap_or_default()),
        }
    }

    /// Convert a scanner span into a byte span.
    fn span_of(&self, span: YamlSpan) -> Span {
        let start = self.byte_of(span.start.index());
        let end = self.byte_of(span.end.index()).max(start);
        Span::new(start, end)
    }
}

/// Incrementally assembles nodes from the event stream.
struct Builder<'offsets> {
    /// Character-to-byte mapping for the source being indexed.
    offsets: &'offsets ByteOffsets,
    /// The source being indexed, for narrowing scalar spans.
    text: &'offsets str,
    /// Open collections, innermost last.
    stack: Vec<Frame>,
    /// The completed root node.
    root: Option<Node>,
    /// Nodes recorded against their anchor identifier, for alias resolution.
    anchors: HashMap<usize, Node>,
}

/// One open collection while its children are being read.
struct Frame {
    /// Byte offset where the collection started.
    start: usize,
    /// The anchor identifier the collection was declared with, if any.
    anchor: usize,
    /// Whether the frame is a mapping rather than a sequence.
    is_mapping: bool,
    /// Completed child nodes, in authored order.
    children: Vec<Node>,
}

impl<'offsets> Builder<'offsets> {
    /// Start a builder for a source indexed by `offsets`.
    fn new(offsets: &'offsets ByteOffsets, text: &'offsets str) -> Self {
        Self {
            offsets,
            text,
            stack: Vec::new(),
            root: None,
            anchors: HashMap::new(),
        }
    }

    /// Consume one event, reporting whether the stream is finished.
    fn accept(&mut self, event: &Event<'_>, span: YamlSpan) -> bool {
        match event {
            Event::Scalar(value, style, anchor, _) => {
                let presentation = convert_style(*style);
                let node = Node {
                    span: scalar_span::narrow(self.text, self.offsets.span_of(span), presentation),
                    kind: NodeKind::Scalar {
                        value: value.to_string(),
                        style: presentation,
                    },
                };
                self.push_node(node, *anchor);
            }
            Event::Alias(anchor) => {
                let resolved = self.anchors.get(anchor).cloned();
                let node = resolved.map_or_else(
                    || Node {
                        span: self.offsets.span_of(span),
                        kind: NodeKind::Scalar {
                            value: String::new(),
                            style: ScalarStyle::Plain,
                        },
                    },
                    |anchored| Node {
                        span: self.offsets.span_of(span),
                        kind: anchored.kind,
                    },
                );
                self.push_node(node, 0);
            }
            Event::SequenceStart(anchor, _) => self.open(span, *anchor, false),
            Event::MappingStart(anchor, _) => self.open(span, *anchor, true),
            Event::SequenceEnd | Event::MappingEnd => self.close(span),
            Event::StreamEnd => return true,
            Event::Nothing | Event::StreamStart | Event::DocumentStart(_) | Event::DocumentEnd => {}
        }
        false
    }

    /// Open a collection frame.
    fn open(&mut self, span: YamlSpan, anchor: usize, is_mapping: bool) {
        self.stack.push(Frame {
            start: self.offsets.span_of(span).start,
            anchor,
            is_mapping,
            children: Vec::new(),
        });
    }

    /// Close the innermost collection frame and attach it to its parent.
    fn close(&mut self, span: YamlSpan) {
        let Some(frame) = self.stack.pop() else {
            return;
        };
        let end = self.offsets.span_of(span).end.max(frame.start);
        let node = Node {
            span: Span::new(frame.start, end),
            kind: if frame.is_mapping {
                NodeKind::Mapping(pair_entries(frame.children))
            } else {
                NodeKind::Sequence(frame.children)
            },
        };
        self.push_node(node, frame.anchor);
    }

    /// Attach a completed node to its parent, or record it as the root.
    fn push_node(&mut self, node: Node, anchor: usize) {
        if anchor != 0 {
            self.anchors.insert(anchor, node.clone());
        }
        match self.stack.last_mut() {
            Some(frame) => frame.children.push(node),
            None => {
                self.root.get_or_insert(node);
            }
        }
    }

    /// Report the completed root node.
    fn finish(self) -> Option<Node> {
        self.root
    }
}

/// Fold a mapping's flat child list into key/value entries.
///
/// A trailing child without a value cannot occur in a well-formed stream; it
/// is dropped rather than paired with a placeholder so that no node claims a
/// span it does not own.
fn pair_entries(children: Vec<Node>) -> Vec<Entry> {
    let mut entries = Vec::with_capacity(children.len());
    let mut iter = children.into_iter();
    while let (Some(key), Some(value)) = (iter.next(), iter.next()) {
        entries.push(Entry { key, value });
    }
    entries
}

/// Map the scanner's scalar style onto the linter's three-way distinction.
const fn convert_style(style: YamlStyle) -> ScalarStyle {
    match style {
        YamlStyle::Plain => ScalarStyle::Plain,
        YamlStyle::SingleQuoted | YamlStyle::DoubleQuoted => ScalarStyle::Quoted,
        YamlStyle::Literal | YamlStyle::Folded => ScalarStyle::Block,
    }
}

#[cfg(test)]
#[path = "document_build_tests.rs"]
mod tests;
