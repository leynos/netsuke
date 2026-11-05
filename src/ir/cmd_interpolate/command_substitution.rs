//! Retain local state for one POSIX-style command substitution.
//!
//! Command substitutions have independent quote and parenthesis state, which
//! prevents their contents from changing the context of surrounding recipe
//! markers.

use super::QuoteContext;
use super::posix_lexical::is_word_terminator;

/// Identify one command-substitution delimiter recognised during traversal.
pub(super) enum CommandSubstitutionDelimiter {
    /// Begin a nested `$()` command substitution.
    Start,
    /// Open a grouped expression within the active command substitution.
    NestedOpen,
    /// Close a grouped expression or the active command substitution.
    Close,
}

/// Retain parsing state for one active command substitution.
pub(super) struct CommandSubstitution {
    /// Record the quote state local to this command-substitution body.
    pub(super) quote_context: QuoteContext,
    /// Count this substitution's opening parenthesis and nested grouped forms.
    pub(super) parenthesis_depth: usize,
    /// Record the `$` that opened this substitution for source-range checks.
    pub(super) start: usize,
}

impl CommandSubstitution {
    /// Initialise the state required after a `$(` delimiter.
    pub(super) const fn new(start: usize) -> Self {
        Self {
            quote_context: QuoteContext::Unquoted,
            parenthesis_depth: 1,
            start,
        }
    }
}

/// Report whether `chars[start..end]` contains the `case` reserved word.
///
/// The scan is deliberately lexical: it does not skip quotes, comments, or
/// nested substitutions, so it can report a `case` that is not a keyword. That
/// over-approximation is safe because the caller fails closed on any match
/// rather than guessing which `)` closes the substitution.
pub(super) fn contains_case_keyword(chars: &[char], start: usize, end: usize) -> bool {
    (start..end).any(|pos| is_case_keyword_at(chars, pos))
}

/// Report whether the four characters at `pos` spell `case` as a whole word.
fn is_case_keyword_at(chars: &[char], pos: usize) -> bool {
    const KEYWORD: [char; 4] = ['c', 'a', 's', 'e'];
    if chars.get(pos..pos + KEYWORD.len()) != Some(KEYWORD.as_slice()) {
        return false;
    }
    let starts_word = pos == 0 || chars.get(pos - 1).is_some_and(|ch| is_word_terminator(*ch));
    let ends_word = chars
        .get(pos + KEYWORD.len())
        .is_none_or(|ch| is_word_terminator(*ch));
    starts_word && ends_word
}
