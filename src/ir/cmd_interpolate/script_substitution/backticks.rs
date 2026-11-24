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
    /// Whether a `\\` pair left a backslash active in the body's own parse.
    ///
    /// The delimiter scan removes the backslash of a `\` before a dollar
    /// sign, a backtick, or a backslash, and the body is then parsed again.
    /// A `\\` pair therefore leaves one backslash for that parse, which
    /// escapes the character that reaches it next; that character must not be
    /// read as a quote, a frame delimiter, or a substitution opener.
    pending_residual_escape: bool,
}

impl BacktickRegion {
    /// Initialise the record for a backtick opened in `outer_context`.
    pub(super) const fn new(outer_context: ShellQuoteContext, owner_depth: usize) -> Self {
        Self {
            outer_context,
            owner_depth,
            body_context: ShellQuoteContext::Unquoted,
            pending_residual_escape: false,
        }
    }

    /// Record that a `\\` pair left a backslash for the body's own parse.
    pub(super) const fn mark_residual_escape(&mut self) {
        self.pending_residual_escape = true;
    }

    /// Take the pending residual backslash, reporting whether it was set.
    pub(super) const fn take_residual_escape(&mut self) -> bool {
        let pending = self.pending_residual_escape;
        self.pending_residual_escape = false;
        pending
    }

    /// Update the body's own quoting and report whether `ch` was a delimiter.
    pub(super) const fn toggle_body_quote(&mut self, ch: char) -> bool {
        toggle_quote_context(&mut self.body_context, ch)
    }

    /// Report whether the backslash at `pos` quotes the next body character.
    ///
    /// The shell finds the closing backtick before it parses the body, so the
    /// body's own escape rule is applied to the whole body first: see
    /// [`escapes_backtick_body_character`], which the caller consults ahead of
    /// this method because it reaches into a `$()` frame opened in the body.
    /// Every other escape belongs to the region whose quoting is active: an
    /// unquoted body lets a backslash quote the next character, the body's
    /// double quotes also protect their own closing delimiter, and its single
    /// quotes give a backslash no special meaning. Keeping the shell's escapes
    /// separate from the body's is what stops an escaped apostrophe from
    /// swallowing the quote that closes a single-quoted body.
    ///
    /// A backslash before a newline is a line continuation in the first two
    /// contexts: the newline is removed rather than ending the command line, so
    /// it must not start a heredoc body that is still queued from an earlier
    /// word. Inside the body's single quotes the newline is literal data.
    pub(super) fn escapes_body_character(&self, chars: &[char], pos: usize) -> bool {
        if escapes_backtick_body_character(chars, pos) {
            return true;
        }
        let Some(next) = chars.get(pos + 1) else {
            return false;
        };
        match self.body_context {
            ShellQuoteContext::SingleQuoted | ShellQuoteContext::AnsiC => false,
            ShellQuoteContext::DoubleQuoted => matches!(next, '"' | '\n'),
            ShellQuoteContext::Unquoted => true,
        }
    }
}

/// Report whether the backtick body's escape rule protects the character after
/// the backslash at `pos`.
///
/// The shell removes the backslash before a dollar sign, a backtick, or a
/// backslash while it searches for the closing backtick, before the body is
/// parsed. That search spans the whole body, so the rule also governs a
/// backtick inside a `$()` frame opened in the body and inside that frame's
/// quotes: the escaped backtick is data there, not a closer. Every other
/// escape is left to the innermost region's own quoting.
pub(super) fn escapes_backtick_body_character(chars: &[char], pos: usize) -> bool {
    matches!(chars.get(pos + 1), Some('$' | '`' | '\\'))
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
