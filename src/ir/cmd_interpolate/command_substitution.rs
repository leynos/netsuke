//! Retain local state for one POSIX-style command substitution.
//!
//! Command substitutions have independent quote and parenthesis state, which
//! prevents their contents from changing the context of surrounding recipe
//! markers.

use super::posix_lexical::is_word_terminator;
use super::{IrGenError, QuoteContext, invalid_command_error};

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

/// Describe the `)` that closed a command substitution.
pub(super) struct SubstitutionBoundary {
    /// Position of the closing `)`.
    pub(super) pos: usize,
    /// The `$` that opened the substitution, when this `)` closed it.
    pub(super) start: Option<usize>,
}

impl SubstitutionBoundary {
    /// Record the `)` at `pos` and the substitution opener it closed, if any.
    pub(super) const fn new(pos: usize, start: Option<usize>) -> Self {
        Self { pos, start }
    }
}

/// Reject a `)#` boundary whose `)` may terminate a `case` pattern.
///
/// A `)` that closes `$()` is part of the surrounding word, so `#` after it is
/// word data. A `)` that terminates a `case` pattern is an operator, so `#`
/// after it starts a comment. The two are lexically identical without the
/// `case` grammar, so a substitution containing `case` fails closed rather than
/// guessing and encoding a later marker for the wrong context.
///
/// The enclosing quote context is deliberately not consulted. A substitution
/// is parsed as its own script, so outer double quotes do not extend into it
/// and cannot make the boundary unambiguous. Callers that model a shell
/// without `case` patterns decide for themselves whether to call this.
///
/// # Errors
///
/// Returns [`IrGenError::InvalidCommand`] when the boundary is ambiguous.
pub(super) fn reject_ambiguous_case_comment(
    chars: &[char],
    template: &str,
    boundary: &SubstitutionBoundary,
) -> Result<(), IrGenError> {
    let Some(start) = boundary.start else {
        return Ok(());
    };
    let is_comment_boundary = chars.get(boundary.pos + 1) == Some(&'#');
    if is_comment_boundary && contains_case_keyword(chars, start, boundary.pos) {
        return Err(invalid_command_error(template.to_owned()));
    }
    Ok(())
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
