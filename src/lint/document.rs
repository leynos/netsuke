//! The authored manifest as a spanned tree.
//!
//! The typed manifest keeps no source positions: YAML is parsed into a
//! `serde_json::Value`, `foreach` expansion rewrites that tree, and typed
//! deserialization discards everything but the values. Lint diagnostics still
//! need to point at a line, and span-scoped suppression needs to know which
//! bytes belong to which node.
//!
//! This module reads the same bytes a second time through the YAML event
//! stream and records where every scalar, sequence, and mapping node sits. It
//! is a position index over the source, not a second opinion about its
//! meaning: a source that fails to parse here has already failed to parse for
//! the compiler, and `netsuke check` reports that parse error instead.

use miette::SourceSpan;

use super::document_build::{ParseFailure, parse_document};

/// A byte range within the manifest source.
///
/// Offsets are byte offsets so they can be handed straight to `miette` and to
/// string slicing, unlike the character indices the YAML scanner reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
    /// Byte offset of the first byte in the range.
    pub start: usize,
    /// Byte offset one past the last byte in the range.
    pub end: usize,
}

impl Span {
    /// Build a span from its inclusive start and exclusive end.
    #[must_use]
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Report the length of the span in bytes.
    #[must_use]
    pub const fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Report whether the span covers no bytes.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.len() == 0
    }

    /// Report whether `other` lies entirely within this span.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.start <= other.start && other.end <= self.end
    }

    /// Report whether `offset` lies within this span.
    #[must_use]
    pub const fn contains_offset(self, offset: usize) -> bool {
        self.start <= offset && offset < self.end
    }
}

impl From<Span> for SourceSpan {
    fn from(span: Span) -> Self {
        Self::new(span.start.into(), span.len())
    }
}

/// The YAML presentation style of a scalar.
///
/// Rules that reason about shell text need this: a `#` inside a quoted or
/// block scalar is content rather than a comment, and a block scalar is where
/// multi-line scripts live.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarStyle {
    /// An unquoted scalar.
    Plain,
    /// A single- or double-quoted scalar.
    Quoted,
    /// A literal (`|`) or folded (`>`) block scalar.
    Block,
}

/// One node of the authored document.
#[derive(Debug, Clone)]
pub struct Node {
    /// Byte range the node occupies in the source.
    pub span: Span,
    /// The node's contents.
    pub kind: NodeKind,
}

/// The three YAML node shapes the manifest schema can contain.
#[derive(Debug, Clone)]
pub enum NodeKind {
    /// A scalar value with its presentation style.
    Scalar {
        /// The scalar's resolved text.
        value: String,
        /// How the scalar was written.
        style: ScalarStyle,
    },
    /// An ordered sequence of nodes.
    Sequence(Vec<Node>),
    /// An ordered list of key/value pairs, preserving authored order.
    Mapping(Vec<Entry>),
}

/// One key/value pair of a mapping node.
#[derive(Debug, Clone)]
pub struct Entry {
    /// The key node, which the manifest schema always writes as a scalar.
    pub key: Node,
    /// The value node.
    pub value: Node,
}

impl Node {
    /// Borrow the scalar text, when this node is a scalar.
    #[must_use]
    pub const fn as_str(&self) -> Option<&str> {
        match &self.kind {
            NodeKind::Scalar { value, .. } => Some(value.as_str()),
            _ => None,
        }
    }

    /// Report the scalar's presentation style, when this node is a scalar.
    #[must_use]
    pub const fn scalar_style(&self) -> Option<ScalarStyle> {
        match &self.kind {
            NodeKind::Scalar { style, .. } => Some(*style),
            _ => None,
        }
    }

    /// Borrow the sequence items, when this node is a sequence.
    #[must_use]
    pub const fn as_sequence(&self) -> Option<&[Self]> {
        match &self.kind {
            NodeKind::Sequence(items) => Some(items.as_slice()),
            _ => None,
        }
    }

    /// Borrow the mapping entries, when this node is a mapping.
    #[must_use]
    pub const fn as_mapping(&self) -> Option<&[Entry]> {
        match &self.kind {
            NodeKind::Mapping(entries) => Some(entries.as_slice()),
            _ => None,
        }
    }

    /// Look up a mapping value by key.
    ///
    /// Duplicate keys are a manifest parse error, so the first match is the
    /// only match in any document that reaches the linter.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Self> {
        self.as_mapping()?
            .iter()
            .find(|entry| entry.key.as_str() == Some(key))
            .map(|entry| &entry.value)
    }

    /// Look up a mapping key node by name, for spans that should point at the
    /// key rather than its value.
    #[must_use]
    pub fn key_node(&self, key: &str) -> Option<&Self> {
        self.as_mapping()?
            .iter()
            .find(|entry| entry.key.as_str() == Some(key))
            .map(|entry| &entry.key)
    }

    /// Iterate the items of a sequence, or nothing when the node is not one.
    ///
    /// The manifest schema writes `targets`, `rules`, and `actions` as
    /// sequences, so callers walking those sections want an empty iterator
    /// rather than a branch when the section is absent or malformed.
    pub fn items(&self) -> impl Iterator<Item = &Self> {
        self.as_sequence().unwrap_or_default().iter()
    }

    /// Visit this node and every node beneath it, outermost first.
    pub fn walk<'a>(&'a self, visit: &mut impl FnMut(&'a Self)) {
        visit(self);
        match &self.kind {
            NodeKind::Scalar { .. } => {}
            NodeKind::Sequence(items) => items.iter().for_each(|item| item.walk(visit)),
            NodeKind::Mapping(entries) => entries.iter().for_each(|entry| {
                entry.key.walk(visit);
                entry.value.walk(visit);
            }),
        }
    }

    /// Find the innermost node that begins on `line`, one-based.
    ///
    /// Suppression directives name a node by the line it starts on, so the
    /// smallest node starting there is the one the directive scopes to.
    #[must_use]
    pub fn innermost_starting_on_line(&self, lines: &LineIndex, line: usize) -> Option<&Self> {
        let mut best: Option<&Self> = None;
        self.walk(&mut |node| {
            if lines.line_of(node.span.start) == line
                && best.is_none_or(|found| found.span.len() >= node.span.len())
            {
                best = Some(node);
            }
        });
        best
    }

    /// Find the innermost node whose span covers `offset`.
    #[must_use]
    pub fn innermost_covering(&self, offset: usize) -> Option<&Self> {
        let mut best: Option<&Self> = None;
        self.walk(&mut |node| {
            if node.span.contains_offset(offset)
                && best.is_none_or(|found| found.span.len() >= node.span.len())
            {
                best = Some(node);
            }
        });
        best
    }
}

/// A parsed manifest source together with its line table.
#[derive(Debug)]
pub struct Document {
    /// The manifest source text.
    text: String,
    /// Byte offsets of each line start.
    lines: LineIndex,
    /// The document's root node, absent when the source holds no document.
    root: Option<Node>,
}

impl Document {
    /// Parse `text` into a spanned document.
    ///
    /// # Errors
    ///
    /// Returns the scanner failure when the source is not well-formed YAML.
    /// The compiler rejects such a source first, so a caller that has already
    /// loaded the manifest will not see this error.
    pub fn parse(text: String) -> Result<Self, ParseFailure> {
        let root = parse_document(&text)?;
        let lines = LineIndex::new(&text);
        Ok(Self { text, lines, root })
    }

    /// Borrow the manifest source text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Borrow the line table.
    #[must_use]
    pub const fn lines(&self) -> &LineIndex {
        &self.lines
    }

    /// Borrow the root node, when the source held a document.
    #[must_use]
    pub const fn root(&self) -> Option<&Node> {
        self.root.as_ref()
    }

    /// Look up a top-level manifest section by name.
    #[must_use]
    pub fn section(&self, name: &str) -> Option<&Node> {
        self.root()?.get(name)
    }

    /// Borrow the source text covered by `span`.
    #[must_use]
    pub fn slice(&self, span: Span) -> &str {
        self.text.get(span.start..span.end).unwrap_or_default()
    }
}

/// Byte offsets of every line start, used to map offsets to line numbers.
#[derive(Debug)]
pub struct LineIndex {
    /// Byte offset of the first byte of each line, in ascending order.
    starts: Vec<usize>,
}

impl LineIndex {
    /// Build a line table for `text`.
    #[must_use]
    pub fn new(text: &str) -> Self {
        let mut starts = vec![0usize];
        starts.extend(
            text.match_indices('\n')
                .map(|(offset, matched)| offset + matched.len()),
        );
        Self { starts }
    }

    /// Report the one-based line containing `offset`.
    #[must_use]
    pub fn line_of(&self, offset: usize) -> usize {
        match self.starts.binary_search(&offset) {
            Ok(index) => index + 1,
            Err(index) => index,
        }
    }

    /// Report the byte range of `line`, one-based, excluding its terminator.
    #[must_use]
    pub fn line_span(&self, line: usize, text: &str) -> Span {
        let Some(start) = line.checked_sub(1).and_then(|index| self.starts.get(index)) else {
            return Span::new(text.len(), text.len());
        };
        let end = self
            .starts
            .get(line)
            .map_or(text.len(), |next| next.saturating_sub(1));
        Span::new(*start, end.max(*start))
    }

    /// Report the number of lines in the indexed text.
    #[must_use]
    pub const fn line_count(&self) -> usize {
        self.starts.len()
    }
}

#[cfg(test)]
#[path = "document_tests.rs"]
mod tests;
