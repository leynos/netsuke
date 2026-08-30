//! Span-scoped lint suppression directives.
//!
//! A directive names the rules it silences and must state a reason, so a
//! suppression documents itself and can be reviewed. There is no blanket
//! disable comment and no `all` selector.
//!
//! The scanner is span-aware rather than line-based because a `#` inside a
//! quoted or block scalar is content: a `script: |` block full of shell
//! comments must not be able to disable rules. Scalar spans come from the same
//! index that gives findings their positions, so the two agree by
//! construction.

use super::document::{Document, Node, NodeKind, Span};

/// Prefix introducing a node-scoped directive.
const NODE_PREFIX: &str = "netsuke-lint:";
/// Prefix introducing a file-scoped directive.
const FILE_PREFIX: &str = "netsuke-lint-file:";
/// Verb every directive uses.
const ALLOW_VERB: &str = "allow";
/// Separator between the rule list and the mandatory reason.
const REASON_SEPARATOR: &str = "--";

/// How far a directive reaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// The directive applies to one node's span.
    Node(Span),
    /// The directive applies to the whole manifest.
    File,
    /// The directive named no node, so it silences nothing.
    Unresolved,
}

/// One parsed `netsuke-lint` comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Directive {
    /// Byte range of the directive comment itself, for its own diagnostics.
    pub span: Span,
    /// One-based line the directive was written on.
    pub line: usize,
    /// Rule names the directive silences, in the order written.
    pub rules: Vec<String>,
    /// The stated reason, absent when the author omitted it.
    pub reason: Option<String>,
    /// What the directive applies to.
    pub scope: Scope,
}

impl Directive {
    /// Report whether this directive silences a finding at `span`.
    ///
    /// Containment is decided by where the finding starts. A collection node's
    /// reported span can run past its own declaration, so requiring the whole
    /// span to fit would let an over-wide end escape a directive that plainly
    /// governs the declaration the finding is about.
    #[must_use]
    pub fn covers(&self, span: Option<Span>) -> bool {
        match self.scope {
            Scope::File => true,
            Scope::Unresolved => false,
            Scope::Node(node) => span.is_some_and(|found| node.contains_offset(found.start)),
        }
    }

    /// Report whether this directive names `rule`.
    #[must_use]
    pub fn names(&self, rule: &str) -> bool {
        self.rules.iter().any(|entry| entry == rule)
    }
}

/// Collect every directive in `doc`, resolved to the node it scopes to.
#[must_use]
pub fn collect(doc: &Document) -> Vec<Directive> {
    let scalars = scalar_spans(doc);
    let mut directives: Vec<Directive> = Vec::new();
    for (line, text) in doc.text().lines().enumerate() {
        let line_span = doc.lines().line_span(line.saturating_add(1), doc.text());
        let Some(found) = find_comment(text, line_span.start, &scalars) else {
            continue;
        };
        if let Some(directive) = parse_comment(doc, line.saturating_add(1), &found) {
            directives.push(directive);
        }
    }
    directives
}

/// A comment found on one line, with the byte offset of its `#`.
struct Comment<'a> {
    /// Byte offset of the `#` that opens the comment.
    hash_offset: usize,
    /// Comment body with the leading `#` and surrounding space removed.
    body: &'a str,
    /// Whether manifest content precedes the comment on the same line.
    has_leading_content: bool,
}

/// Locate the first comment on `text` that is not inside a scalar.
fn find_comment<'a>(text: &'a str, line_start: usize, scalars: &[Span]) -> Option<Comment<'a>> {
    text.match_indices('#')
        .find(|(index, _)| {
            let offset = line_start.saturating_add(*index);
            opens_comment(text, *index) && !scalars.iter().any(|span| span.contains_offset(offset))
        })
        .map(|(index, _)| Comment {
            hash_offset: line_start.saturating_add(index),
            body: text
                .get(index.saturating_add(1)..)
                .unwrap_or_default()
                .trim(),
            has_leading_content: !text.get(..index).unwrap_or_default().trim().is_empty(),
        })
}

/// Report whether the `#` at `index` opens a comment rather than sitting
/// inside a word, which YAML treats as content.
fn opens_comment(text: &str, index: usize) -> bool {
    index == 0
        || text
            .get(..index)
            .and_then(|prefix| prefix.chars().next_back())
            .is_some_and(char::is_whitespace)
}

/// Parse a comment into a directive, ignoring comments that are not ours.
fn parse_comment(doc: &Document, line: usize, comment: &Comment<'_>) -> Option<Directive> {
    let (is_file, rest) = strip_prefix(comment.body)?;
    let span = Span::new(
        comment.hash_offset,
        comment
            .hash_offset
            .saturating_add(comment.body.len())
            .saturating_add(1),
    );
    let (rules, reason) = parse_body(rest);
    let scope = if is_file {
        Scope::File
    } else {
        resolve_scope(doc, line, comment.has_leading_content)
    };
    Some(Directive {
        span,
        line,
        rules,
        reason,
        scope,
    })
}

/// Strip the directive prefix, reporting whether it was the file-scoped form.
fn strip_prefix(body: &str) -> Option<(bool, &str)> {
    if let Some(rest) = body.strip_prefix(FILE_PREFIX) {
        return Some((true, rest.trim_start()));
    }
    body.strip_prefix(NODE_PREFIX)
        .map(|rest| (false, rest.trim_start()))
}

/// Split a directive body into its rule list and its reason.
///
/// The `allow` verb is optional in the parse so that a mistyped directive
/// still yields rule names, which lets `unknown-suppression` explain what went
/// wrong instead of the directive being silently ignored.
fn parse_body(body: &str) -> (Vec<String>, Option<String>) {
    let rest = body.strip_prefix(ALLOW_VERB).unwrap_or(body).trim();
    let (rules_text, reason) = match rest.split_once(REASON_SEPARATOR) {
        Some((rules, reason)) if !reason.trim().is_empty() => {
            (rules, Some(reason.trim().to_owned()))
        }
        Some((rules, _)) => (rules, None),
        None => (rest, None),
    };
    let rules = rules_text
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect();
    (rules, reason)
}

/// Resolve which manifest block a node-scoped directive applies to.
///
/// Scoping follows YAML indentation rather than the node tree, because that is
/// how a reader sees the file: a directive governs the declaration it sits
/// above or beside, together with everything indented beneath it. A trailing
/// directive scopes to its own line's block; a directive alone on its line
/// scopes to the block starting at the next line that is neither blank nor
/// another comment, so a run of directives above one declaration all apply to
/// that declaration.
fn resolve_scope(doc: &Document, line: usize, has_leading_content: bool) -> Scope {
    let anchor = if has_leading_content {
        Some(line)
    } else {
        next_content_line(doc, line)
    };
    anchor.map_or(Scope::Unresolved, |start| {
        Scope::Node(block_span(doc, start))
    })
}

/// Report the first line after `line` that holds manifest content.
fn next_content_line(doc: &Document, line: usize) -> Option<usize> {
    ((line.saturating_add(1))..=doc.lines().line_count()).find(|candidate| {
        let text = line_text(doc, *candidate).trim();
        !text.is_empty() && !text.starts_with('#')
    })
}

/// Report the byte range of the block that begins at `line`.
///
/// The block is that line plus every following line that is blank or indented
/// further than it. Trailing blank lines are not included, so a directive on
/// the last line of one declaration cannot reach into the next.
fn block_span(doc: &Document, line: usize) -> Span {
    let start = doc.lines().line_span(line, doc.text());
    let indent = indent_of(line_text(doc, line));
    let mut end = start.end;
    for candidate in (line.saturating_add(1))..=doc.lines().line_count() {
        let span = doc.lines().line_span(candidate, doc.text());
        let text = doc.slice(span);
        if text.trim().is_empty() {
            continue;
        }
        if indent_of(text) <= indent {
            break;
        }
        // Spans are absolute offsets, so extending to a later indented line
        // also covers the blank lines skipped on the way there.
        end = span.end;
    }
    Span::new(start.start, end.max(start.end))
}

/// Borrow the text of one line, excluding its terminator.
fn line_text(doc: &Document, line: usize) -> &str {
    doc.slice(doc.lines().line_span(line, doc.text()))
}

/// Report the leading-space count of a line.
fn indent_of(line: &str) -> usize {
    line.chars()
        .take_while(|character| *character == ' ')
        .count()
}

/// Collect the spans of every scalar in the document.
///
/// Quoted and block scalars can contain a `#`, and plain scalars cannot
/// contain a comment at all, so treating every scalar span as content is both
/// sound and simple.
fn scalar_spans(doc: &Document) -> Vec<Span> {
    let mut spans = Vec::new();
    if let Some(root) = doc.root() {
        root.walk(&mut |node: &Node| {
            if matches!(node.kind, NodeKind::Scalar { .. }) {
                spans.push(node.span);
            }
        });
    }
    spans
}

#[cfg(test)]
#[path = "suppress_tests.rs"]
mod tests;
