//! Track the shell region that owns an open backtick command substitution.
//!
//! A backtick saves the quote context of the region around it and restores that
//! context when the closing backtick arrives. Keeping that record, its body's
//! quoting, and the escape rules a body obeys together keeps the parent
//! traversal within the repository's module-size cap.

use super::ShellQuoteContext;

/// Record the shell region that opened a backtick command substitution.
///
/// The saved context belongs to the region at the recorded `$()` depth, not to
/// the script-level field: restoring it at script level would leave a later
/// marker outside the substitution using the quoting of the backtick body,
/// which for an unquoted path can leave a command separator executable.
#[derive(Clone, Copy)]
pub(super) struct BacktickRegion {
    /// Quote context to restore when the closing backtick is reached.
    pub(super) outer_context: ShellQuoteContext,
    /// `$()` frame depth at the opening backtick; zero means script level.
    pub(super) owner_depth: usize,
    /// Quoting of the backtick body itself.
    ///
    /// The body is its own shell region: quotes in it protect delimiters, but
    /// they belong to the body rather than to the region the backtick will
    /// restore. Tracking them separately keeps a quoted `)` inside the body
    /// literal instead of letting it close the owning `$()`.
    pub(super) body_context: ShellQuoteContext,
}

impl BacktickRegion {
    /// Initialise the record for a backtick opened in `outer_context`.
    pub(super) const fn new(outer_context: ShellQuoteContext, owner_depth: usize) -> Self {
        Self {
            outer_context,
            owner_depth,
            body_context: ShellQuoteContext::Unquoted,
        }
    }

    /// Update the body's own quoting and report whether `ch` was a delimiter.
    pub(super) const fn toggle_body_quote(&mut self, ch: char) -> bool {
        toggle_quote_context(&mut self.body_context, ch)
    }

    /// Report whether the backslash at `pos` quotes the next body character.
    ///
    /// The shell finds the closing backtick before it parses the body, so a
    /// backslash protects a following dollar sign, backtick, or backslash even
    /// where the body's own quoting would make a backslash literal. Every other
    /// escape belongs to the body's own quoting: an unquoted body lets a
    /// backslash quote the next character, the body's double quotes also
    /// protect their own closing delimiter, and its single quotes give a
    /// backslash no special meaning. Keeping the shell's escapes separate from
    /// the body's is what stops an escaped apostrophe from swallowing the quote
    /// that closes a single-quoted body.
    ///
    /// A backslash before a newline is a line continuation in the first two
    /// contexts: the newline is removed rather than ending the command line, so
    /// it must not start a heredoc body that is still queued from an earlier
    /// word. Inside the body's single quotes the newline is literal data.
    pub(super) fn escapes_body_character(&self, chars: &[char], pos: usize) -> bool {
        let Some(next) = chars.get(pos + 1) else {
            return false;
        };
        match self.body_context {
            ShellQuoteContext::SingleQuoted => matches!(next, '$' | '`' | '\\'),
            ShellQuoteContext::DoubleQuoted => matches!(next, '$' | '`' | '\\' | '"' | '\n'),
            ShellQuoteContext::Unquoted => true,
        }
    }
}

/// Apply one quote delimiter to `context` and report whether `ch` was one.
///
/// A quote delimiter only closes the quote it opened, so a `"` inside single
/// quotes and a `'` inside double quotes are ordinary characters.
pub(super) const fn toggle_quote_context(context: &mut ShellQuoteContext, ch: char) -> bool {
    match (*context, ch) {
        (ShellQuoteContext::Unquoted, '\'') => {
            *context = ShellQuoteContext::SingleQuoted;
            true
        }
        (ShellQuoteContext::Unquoted, '"') => {
            *context = ShellQuoteContext::DoubleQuoted;
            true
        }
        (ShellQuoteContext::SingleQuoted, '\'') | (ShellQuoteContext::DoubleQuoted, '"') => {
            *context = ShellQuoteContext::Unquoted;
            true
        }
        _ => false,
    }
}
