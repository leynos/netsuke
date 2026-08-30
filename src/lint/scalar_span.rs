//! Narrow a scanner-reported scalar span to the scalar's own text.
//!
//! The YAML scanner reports a scalar's span as running to wherever the next
//! token begins, so a trailing comment, the rest of the line, and sometimes the
//! first line of the next declaration fall inside it. That is harmless for the
//! scanner and wrong for a linter: an over-wide span makes a diagnostic
//! underline unrelated text, makes a shell scan treat a closing quote as still
//! open, and makes the suppression scanner mistake a directive for scalar
//! content.

use super::document::{ScalarStyle, Span};

/// Narrow `span` to the scalar it actually covers.
#[must_use]
pub fn narrow(text: &str, span: Span, style: ScalarStyle) -> Span {
    let Some(slice) = text.get(span.start..span.end) else {
        return span;
    };
    let len = match style {
        ScalarStyle::Quoted => quoted_len(slice),
        ScalarStyle::Plain => plain_len(slice),
        ScalarStyle::Block => block_len(text, span, slice),
    };
    Span::new(span.start, span.start.saturating_add(len))
}

/// Report the length of a quoted scalar, up to and including its closing quote.
fn quoted_len(slice: &str) -> usize {
    let mut characters = slice.char_indices();
    let Some((_, quote)) = characters.next() else {
        return slice.len();
    };
    if !matches!(quote, '\'' | '"') {
        return trimmed_len(slice);
    }
    let mut skip_next = false;
    for (index, character) in characters {
        if skip_next {
            skip_next = false;
            continue;
        }
        if escapes_next(character, quote) {
            skip_next = true;
            continue;
        }
        if character != quote {
            continue;
        }
        if is_doubled_quote(slice, index, quote) {
            skip_next = true;
            continue;
        }
        return index.saturating_add(character.len_utf8());
    }
    trimmed_len(slice)
}

/// Report whether `character` escapes the one after it.
///
/// A single-quoted YAML scalar has no backslash escape; a double-quoted one
/// does.
const fn escapes_next(character: char, quote: char) -> bool {
    character == '\\' && quote == '"'
}

/// Report whether the quote at `index` is a doubled, escaped single quote.
fn is_doubled_quote(slice: &str, index: usize, quote: char) -> bool {
    quote == '\''
        && slice
            .get(index.saturating_add(1)..)
            .is_some_and(|rest| rest.starts_with('\''))
}

/// Report the length of a plain scalar, stopping before any trailing comment.
///
/// YAML only starts a comment where a `#` follows whitespace, so a `#` inside a
/// word remains scalar content.
fn plain_len(slice: &str) -> usize {
    let trimmed = slice.trim_end();
    let comment = trimmed
        .char_indices()
        .find(|(index, character)| *character == '#' && preceded_by_space(trimmed, *index));
    comment.map_or_else(
        || trimmed.len(),
        |(index, _)| trimmed.get(..index).unwrap_or_default().trim_end().len(),
    )
}

/// Report whether the character before `index` is a space or tab.
fn preceded_by_space(text: &str, index: usize) -> bool {
    text.get(..index)
        .and_then(|prefix| prefix.chars().next_back())
        .is_some_and(|character| character == ' ' || character == '\t')
}

/// Report the length of a block scalar, clipped to its indented body.
///
/// A block scalar owns its header line plus every following line that is blank
/// or indented further than the header. The scanner's reported end can reach
/// past that into the next declaration.
fn block_len(text: &str, span: Span, slice: &str) -> usize {
    let header_indent = line_indent(text, span.start);
    let mut length = first_line_len(slice);
    let mut pending = length;
    for line in slice
        .get(length..)
        .unwrap_or_default()
        .split_inclusive('\n')
    {
        if line.trim().is_empty() {
            pending = pending.saturating_add(line.len());
            continue;
        }
        if indent_of(line) <= header_indent {
            break;
        }
        pending = pending.saturating_add(line.len());
        length = pending;
    }
    trimmed_len(slice.get(..length).unwrap_or(slice))
}

/// Report the length of the slice's first line, including its terminator.
fn first_line_len(slice: &str) -> usize {
    slice
        .find('\n')
        .map_or(slice.len(), |index| index.saturating_add(1))
}

/// Report the indentation, in characters, of the line containing `offset`.
fn line_indent(text: &str, offset: usize) -> usize {
    let start = text
        .get(..offset)
        .and_then(|prefix| prefix.rfind('\n').map(|index| index.saturating_add(1)))
        .unwrap_or_default();
    indent_of(text.get(start..).unwrap_or_default())
}

/// Report the leading-space count of a line.
fn indent_of(line: &str) -> usize {
    line.chars()
        .take_while(|character| *character == ' ')
        .count()
}

/// Report the length of `slice` with trailing whitespace removed.
fn trimmed_len(slice: &str) -> usize {
    slice.trim_end().len()
}

#[cfg(test)]
#[path = "scalar_span_tests.rs"]
mod tests;
