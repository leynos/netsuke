//! Parse and render the one shell word that declares a heredoc delimiter.
//!
//! A delimiter word has its own small quoting grammar: POSIX quote removal
//! applies, backslashes escape only where the surrounding quote context says
//! they do, and a marker inside the word must be encoded for that context. This
//! module keeps that grammar next to the rendering pass that consumes it, while
//! the parent module tracks where a declaration begins and ends.

use super::super::{CommandBindings, Placeholder, QuoteContext, find_substitution};
use super::{EscapeBeforeMarker, backslash_escapes_marker};

/// Track quote removal within one heredoc delimiter word.
#[derive(Clone, Copy)]
enum DelimiterQuoteContext {
    /// Treat characters as unquoted shell word text.
    Unquoted,
    /// Retain literal text between single quotes.
    Single,
    /// Retain literal text between double quotes.
    Double,
}

/// Parse a heredoc delimiter word and return its quote-removed text and end.
pub(super) fn parse_delimiter(chars: &[char], start: usize) -> Option<(String, usize)> {
    let mut text = String::new();
    let mut quote_context = DelimiterQuoteContext::Unquoted;
    let mut pos = start;
    let mut consumed = false;
    while let Some(ch) = chars.get(pos).copied() {
        if matches!(quote_context, DelimiterQuoteContext::Unquoted) && is_word_terminator(ch) {
            break;
        }
        match (quote_context, ch) {
            (DelimiterQuoteContext::Unquoted, '\'') => {
                quote_context = DelimiterQuoteContext::Single;
                pos += 1;
                consumed = true;
            }
            (DelimiterQuoteContext::Unquoted, '"') => {
                quote_context = DelimiterQuoteContext::Double;
                pos += 1;
                consumed = true;
            }
            (DelimiterQuoteContext::Unquoted, '\\') => {
                let next = *chars.get(pos + 1)?;
                text.push(next);
                pos += 2;
                consumed = true;
            }
            (DelimiterQuoteContext::Single, '\'') | (DelimiterQuoteContext::Double, '"') => {
                quote_context = DelimiterQuoteContext::Unquoted;
                pos += 1;
                consumed = true;
            }
            (DelimiterQuoteContext::Double, '\\') => {
                let next = *chars.get(pos + 1)?;
                if matches!(next, '$' | '`' | '"' | '\\' | '\n') {
                    text.push(next);
                } else {
                    text.push('\\');
                    text.push(next);
                }
                pos += 2;
                consumed = true;
            }
            _ => {
                text.push(ch);
                pos += 1;
                consumed = true;
            }
        }
    }
    consumed.then_some((text, pos))
}

/// Render recipe markers in a heredoc delimiter using that word's quote context.
///
/// # Errors
///
/// Returns [`EscapeBeforeMarker`] when an active backslash directly precedes a
/// marker inside the delimiter word.
pub(super) fn render_delimiter(
    chars: &[char],
    start: usize,
    end: usize,
    bindings: &CommandBindings,
) -> Result<String, EscapeBeforeMarker> {
    let mut rendered = String::new();
    let mut quote_context = DelimiterQuoteContext::Unquoted;
    let mut pos = start;
    while pos < end
        && let Some(ch) = chars.get(pos).copied()
    {
        let is_single_quoted = matches!(quote_context, DelimiterQuoteContext::Single);
        if backslash_escapes_marker(chars, pos, is_single_quoted, Some(end)) {
            return Err(EscapeBeforeMarker);
        }
        if let Some(next) = escaped_delimiter_character(chars, pos, end, quote_context) {
            rendered.push(ch);
            rendered.push(next);
            pos += 2;
            continue;
        }
        if let Some((placeholder, skip)) = matching_delimiter_marker(chars, pos, end) {
            rendered.push_str(bindings.substitution(placeholder, quote_context.into()));
            pos += skip;
            continue;
        }
        rendered.push(ch);
        update_delimiter_quote_context(&mut quote_context, ch);
        pos += 1;
    }
    Ok(rendered)
}

/// Return the escaped character that must retain the current delimiter context.
fn escaped_delimiter_character(
    chars: &[char],
    pos: usize,
    end: usize,
    quote_context: DelimiterQuoteContext,
) -> Option<char> {
    let ch = chars.get(pos).copied()?;
    if ch != '\\' || matches!(quote_context, DelimiterQuoteContext::Single) {
        return None;
    }
    let next_pos = pos.checked_add(1)?;
    if next_pos >= end {
        return None;
    }
    chars.get(next_pos).copied()
}

/// Return a marker wholly contained in the heredoc delimiter word.
fn matching_delimiter_marker(
    chars: &[char],
    pos: usize,
    end: usize,
) -> Option<(Placeholder, usize)> {
    let (placeholder, skip) = find_substitution(chars, pos)?;
    (pos + skip <= end).then_some((placeholder, skip))
}

/// Update the quote state that selects heredoc-delimiter path encoding.
const fn update_delimiter_quote_context(quote_context: &mut DelimiterQuoteContext, ch: char) {
    match (*quote_context, ch) {
        (DelimiterQuoteContext::Unquoted, '\'') => *quote_context = DelimiterQuoteContext::Single,
        (DelimiterQuoteContext::Unquoted, '"') => *quote_context = DelimiterQuoteContext::Double,
        (DelimiterQuoteContext::Single, '\'') | (DelimiterQuoteContext::Double, '"') => {
            *quote_context = DelimiterQuoteContext::Unquoted;
        }
        _ => {}
    }
}

impl From<DelimiterQuoteContext> for QuoteContext {
    /// Convert delimiter-local quoting into the corresponding path-encoding context.
    fn from(context: DelimiterQuoteContext) -> Self {
        match context {
            DelimiterQuoteContext::Unquoted => Self::Unquoted,
            DelimiterQuoteContext::Single => Self::Single('\''),
            DelimiterQuoteContext::Double => Self::Double('"'),
        }
    }
}

/// Report whether `ch` terminates an unquoted shell word.
const fn is_word_terminator(ch: char) -> bool {
    ch.is_whitespace() || matches!(ch, ';' | '|' | '&' | '(' | ')' | '<' | '>')
}
