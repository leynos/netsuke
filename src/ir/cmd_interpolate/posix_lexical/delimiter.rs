//! Parse and render the one shell word that declares a heredoc delimiter.
//!
//! A delimiter word has its own small quoting grammar: POSIX quote removal
//! applies, backslashes escape only where the surrounding quote context says
//! they do, and a marker inside the word must be encoded for that context. This
//! module keeps that grammar next to the rendering pass that consumes it, while
//! the parent module tracks where a declaration begins and ends.
//!
//! A word the tracker cannot reproduce exactly is rejected rather than guessed
//! at: a mis-tracked terminator would let marker lowering resume inside text
//! the shell still holds as heredoc data.

use super::super::{CommandBindings, Placeholder, QuoteContext, find_substitution};
use super::{DelimiterRejection, EscapeBeforeMarker, backslash_escapes_marker, is_word_terminator};

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

/// Describe one parsed heredoc delimiter word.
pub(super) struct ParsedDelimiter {
    /// Store the quote-removed text the shell compares against a body line.
    pub(super) text: String,
    /// Record the source index one past the word.
    pub(super) end: usize,
    /// Record whether any part of the word was quoted or escaped.
    ///
    /// A quoted delimiter suppresses expansion in the body, and with it the
    /// backslash-newline joining the shell performs before terminator matching.
    pub(super) is_quoted: bool,
}

/// Parse a heredoc delimiter word and return its quote-removed text and end.
///
/// Only POSIX blanks, newlines, and shell operators end an unquoted word.
/// Unicode whitespace, CR, vertical tab, and form feed stay inside the
/// delimiter, matching the shell's byte-oriented tokenization. Forms whose
/// quote removal or expansion the tracker cannot reproduce are rejected so a
/// later marker cannot be lowered into text the shell still holds as heredoc
/// data. A rejected word also covers a leading unquoted `#`, which the shell
/// reads as a comment and therefore as a missing delimiter.
///
/// # Errors
///
/// Returns the [`DelimiterRejection`] naming the syntax that cannot be modelled.
pub(super) fn parse_delimiter(
    chars: &[char],
    start: usize,
) -> Result<ParsedDelimiter, DelimiterRejection> {
    if chars.get(start) == Some(&'#') {
        return Err(DelimiterRejection::MissingDelimiter);
    }
    let mut word = DelimiterWord::new(start);
    while let Some(ch) = chars.get(word.pos).copied() {
        if word.is_terminated_by(ch) {
            break;
        }
        word.consume(ch, chars)?;
    }
    word.finish()
}

/// Accumulate one heredoc delimiter word while applying shell quote removal.
struct DelimiterWord {
    /// Store the quote-removed text the shell compares against a source line.
    text: String,
    /// Track the quoting that decides how the next character is read.
    quote_context: DelimiterQuoteContext,
    /// Record the next source position to read.
    pos: usize,
    /// Record whether any character or quote was consumed.
    consumed: bool,
    /// Record whether any part of the word was quoted or escaped.
    is_quoted: bool,
}

impl DelimiterWord {
    /// Start an empty delimiter word at source position `start`.
    const fn new(start: usize) -> Self {
        Self {
            text: String::new(),
            quote_context: DelimiterQuoteContext::Unquoted,
            pos: start,
            consumed: false,
            is_quoted: false,
        }
    }

    /// Report whether an unquoted `ch` ends the delimiter word.
    const fn is_terminated_by(&self, ch: char) -> bool {
        matches!(self.quote_context, DelimiterQuoteContext::Unquoted) && is_word_terminator(ch)
    }

    /// Apply one source character, stopping at forms the tracker cannot model.
    fn consume(&mut self, ch: char, chars: &[char]) -> Result<(), DelimiterRejection> {
        match (self.quote_context, ch) {
            (DelimiterQuoteContext::Unquoted, '\'') => {
                self.quote_context = DelimiterQuoteContext::Single;
                self.is_quoted = true;
                self.advance_one();
            }
            (DelimiterQuoteContext::Unquoted, '"') => {
                self.quote_context = DelimiterQuoteContext::Double;
                self.is_quoted = true;
                self.advance_one();
            }
            (DelimiterQuoteContext::Unquoted | DelimiterQuoteContext::Double, '$') => {
                return Err(DelimiterRejection::UnsupportedExpansion);
            }
            (DelimiterQuoteContext::Unquoted | DelimiterQuoteContext::Double, '`') => {
                return Err(DelimiterRejection::UnsupportedCommandSubstitution);
            }
            (DelimiterQuoteContext::Single, '\'') | (DelimiterQuoteContext::Double, '"') => {
                self.quote_context = DelimiterQuoteContext::Unquoted;
                self.advance_one();
            }
            (DelimiterQuoteContext::Unquoted, '\\') => {
                self.is_quoted = true;
                self.text.push(escaped_character(chars, self.pos)?);
                self.advance_two();
            }
            (DelimiterQuoteContext::Double, '\\') => {
                self.is_quoted = true;
                let escaped = escaped_character(chars, self.pos)?;
                if matches!(escaped, '$' | '`' | '"' | '\\') {
                    self.text.push(escaped);
                } else {
                    self.text.push('\\');
                    self.text.push(escaped);
                }
                self.advance_two();
            }
            _ => {
                self.text.push(ch);
                self.advance_one();
            }
        }
        Ok(())
    }

    /// Finish the word, rejecting an unclosed quote or an empty word.
    fn finish(self) -> Result<ParsedDelimiter, DelimiterRejection> {
        if !matches!(self.quote_context, DelimiterQuoteContext::Unquoted) {
            return Err(DelimiterRejection::UnclosedQuote);
        }
        if self.consumed {
            Ok(ParsedDelimiter {
                text: self.text,
                end: self.pos,
                is_quoted: self.is_quoted,
            })
        } else {
            Err(DelimiterRejection::MissingDelimiter)
        }
    }

    /// Copy the current character and move past it.
    const fn advance_one(&mut self) {
        self.pos += 1;
        self.consumed = true;
    }

    /// Move past the current character and the one it escapes.
    const fn advance_two(&mut self) {
        self.pos += 2;
        self.consumed = true;
    }
}

/// Read the character escaped by the backslash at `pos`.
///
/// Rejects a backslash with nothing after it and an unquoted backslash-newline
/// line continuation, which would join the delimiter to the next source line.
fn escaped_character(chars: &[char], pos: usize) -> Result<char, DelimiterRejection> {
    let Some(escaped) = chars.get(pos + 1).copied() else {
        return Err(DelimiterRejection::TrailingBackslash);
    };
    if escaped == '\n' {
        return Err(DelimiterRejection::LineContinuation);
    }
    Ok(escaped)
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

/// Return the terminator and quoting of a rendered delimiter word.
///
/// Lowering runs before the shell, and an inserted path may change how the
/// shell reads the word, so the rendered text is re-parsed with the same
/// grammar rather than trusting the source parse.
///
/// # Errors
///
/// Returns [`DelimiterRejection::RenderedDelimiterInvalid`] when lowering left
/// text outside the word, or produced a word the parser rejects.
pub(super) fn rendered_terminator(rendered: &str) -> Result<ParsedDelimiter, DelimiterRejection> {
    let rendered_chars: Vec<_> = rendered.chars().collect();
    match parse_delimiter(&rendered_chars, 0) {
        Ok(parsed) if parsed.end == rendered_chars.len() => Ok(parsed),
        Ok(_) | Err(_) => Err(DelimiterRejection::RenderedDelimiterInvalid),
    }
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
